package main

// Ecosystem surface — the agent-native front door of the Ares trading
// ecosystem, served by the Ọmọ Kọ́dà ops gateway.
//
//   - GET /ecosystem     — one-shot live aggregation across the playground:
//     Vantage council (verdicts/calibration/overview), ares-signal-fusion
//     top picks, ares-poolhealth key/proxy state, and wallet_intel.db stats.
//     Pure JSON; every source is independent (a failing source is reported
//     in "source_errors" and nulled, never allowed to sink the whole call).
//   - GET /v1/token/{address} — the "ask Omokoda about token X" intake:
//     full intelligence bundle for one mint (picks score, council verdicts
//     for its symbol, whale activity from wallet_intel.db, pool health).
//   - GET /ecosystem/ui  — minimal human-readable page over the same data.
//
// Configuration (all env-optional):
//
//	VANTAGE_URL       council base URL            (default http://127.0.0.1:8001)
//	VANTAGE_KEY       agent key for council       (default: read VANTAGE_KEY_FILE)
//	VANTAGE_KEY_FILE  file holding the key        (default /root/.vantage_key)
//	PICKS_URL         ares-signal-fusion base     (default http://127.0.0.1:8003)
//	POOLHEALTH_URL    ares-poolhealth base        (default http://127.0.0.1:8004)
//	WALLET_INTEL_DB   wallet_intel sqlite path    (default /opt/ares/wallet_intel/wallet_intel.db)
//
// Read-only by construction: no upstream write is ever issued, and the
// sqlite handle is opened in read-only mode. PAPER only — no trades.

import (
	"context"
	"database/sql"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"os"
	"sort"
	"strings"
	"time"

	_ "github.com/mattn/go-sqlite3" // read-only sqlite access to wallet_intel.db
)

const (
	ecoSvcName = "omokoda-ops/ecosystem"

	defaultVantageURL     = "http://127.0.0.1:8001"
	defaultVantageKeyFile = "/root/.vantage_key"
	defaultPicksURL       = "http://127.0.0.1:8003"
	defaultPoolHealthURL  = "http://127.0.0.1:8004"
	defaultWalletIntelDB  = "/opt/ares/wallet_intel/wallet_intel.db"

	ecoUpstreamTimeout = 6 * time.Second
	ecoMaxBodyBytes    = 8 << 20 // 8 MiB safety cap per upstream body
	ecoVerdictsTop     = 10      // recent verdicts surfaced on /ecosystem
	ecoPicksTop        = 5       // top picks surfaced on /ecosystem
	ecoIntakePicksPool = 50      // picks scanned for a token-intake match
	ecoIntakeVerdicts  = 5       // verdicts kept per token intake
)

var ecoHTTP = &http.Client{Timeout: ecoUpstreamTimeout}

// --- configuration -------------------------------------------------------

func envOr(key, fallback string) string {
	if v := os.Getenv(key); v != "" {
		return v
	}
	return fallback
}

func ecoVantageURL() string    { return envOr("VANTAGE_URL", defaultVantageURL) }
func ecoPicksURL() string      { return envOr("PICKS_URL", defaultPicksURL) }
func ecoPoolHealthURL() string { return envOr("POOLHEALTH_URL", defaultPoolHealthURL) }
func ecoWalletIntelDB() string { return envOr("WALLET_INTEL_DB", defaultWalletIntelDB) }

// ecoVantageKey resolves the council agent key from the environment or the
// key file. Never logged, never exposed in responses.
func ecoVantageKey() string {
	if k := os.Getenv("VANTAGE_KEY"); k != "" {
		return k
	}
	b, err := os.ReadFile(envOr("VANTAGE_KEY_FILE", defaultVantageKeyFile))
	if err != nil {
		return ""
	}
	return strings.TrimSpace(string(b))
}

// --- upstream fetch helpers ----------------------------------------------

func ecoFetchJSON(ctx context.Context, url string, hdr map[string]string) (json.RawMessage, error) {
	req, err := http.NewRequestWithContext(ctx, http.MethodGet, url, nil)
	if err != nil {
		return nil, err
	}
	for k, v := range hdr {
		req.Header.Set(k, v)
	}
	resp, err := ecoHTTP.Do(req)
	if err != nil {
		return nil, err
	}
	defer resp.Body.Close()
	body, err := io.ReadAll(io.LimitReader(resp.Body, ecoMaxBodyBytes))
	if err != nil {
		return nil, err
	}
	if resp.StatusCode != http.StatusOK {
		return nil, fmt.Errorf("upstream %s -> HTTP %d", url, resp.StatusCode)
	}
	if !json.Valid(body) {
		return nil, fmt.Errorf("upstream %s returned invalid JSON", url)
	}
	return json.RawMessage(body), nil
}

// ecoCouncilHeaders returns the auth header set for Vantage council calls.
func ecoCouncilHeaders() map[string]string {
	if k := ecoVantageKey(); k != "" {
		return map[string]string{"X-Agent-Key": k}
	}
	return nil
}

// ecoCouncilVerdicts fetches /api/council/verdicts and keeps the `top`
// newest verdicts (sorted by id, descending).
func ecoCouncilVerdicts(ctx context.Context, top int) (json.RawMessage, error) {
	raw, err := ecoFetchJSON(ctx, ecoVantageURL()+"/api/council/verdicts", ecoCouncilHeaders())
	if err != nil {
		return nil, err
	}
	var verdicts []map[string]any
	if err := json.Unmarshal(raw, &verdicts); err != nil {
		return nil, fmt.Errorf("council verdicts: %w", err)
	}
	sort.SliceStable(verdicts, func(i, j int) bool {
		return verdictID(verdicts[i]) > verdictID(verdicts[j])
	})
	if len(verdicts) > top {
		verdicts = verdicts[:top]
	}
	if len(verdicts) == 0 {
		return json.RawMessage("[]"), nil
	}
	return json.Marshal(verdicts)
}

func verdictID(v map[string]any) int64 {
	switch id := v["id"].(type) {
	case float64:
		return int64(id)
	case int64:
		return id
	}
	return 0
}

// ecoVerdictsForSymbol returns verdicts whose symbol matches, newest first.
func ecoVerdictsForSymbol(ctx context.Context, symbol string, top int) (json.RawMessage, error) {
	if strings.TrimSpace(symbol) == "" {
		return json.RawMessage("[]"), nil
	}
	raw, err := ecoFetchJSON(ctx, ecoVantageURL()+"/api/council/verdicts", ecoCouncilHeaders())
	if err != nil {
		return nil, err
	}
	var verdicts []map[string]any
	if err := json.Unmarshal(raw, &verdicts); err != nil {
		return nil, fmt.Errorf("council verdicts: %w", err)
	}
	keep := make([]map[string]any, 0, 4)
	for _, v := range verdicts {
		if sym, ok := v["symbol"].(string); ok && strings.EqualFold(sym, symbol) {
			keep = append(keep, v)
		}
	}
	sort.SliceStable(keep, func(i, j int) bool {
		return verdictID(keep[i]) > verdictID(keep[j])
	})
	if len(keep) > top {
		keep = keep[:top]
	}
	if len(keep) == 0 {
		return json.RawMessage("[]"), nil
	}
	return json.Marshal(keep)
}

// ecoPicksByAddress fetches the picks pool and returns the entry matching
// the token address (nil when absent).
func ecoPicksByAddress(ctx context.Context, address string) (json.RawMessage, error) {
	raw, err := ecoFetchJSON(
		ctx,
		fmt.Sprintf("%s/api/picks?limit=%d", ecoPicksURL(), ecoIntakePicksPool),
		nil,
	)
	if err != nil {
		return nil, err
	}
	var payload struct {
		Picks []map[string]any `json:"picks"`
	}
	if err := json.Unmarshal(raw, &payload); err != nil {
		return nil, fmt.Errorf("picks payload: %w", err)
	}
	for _, p := range payload.Picks {
		if ta, ok := p["token_addr"].(string); ok && strings.EqualFold(ta, address) {
			return json.Marshal(p)
		}
	}
	return nil, nil // not in the pool — valid outcome, not an error
}

// ecoPicksTopN fetches /api/picks?limit=N and returns the payload as-is.
func ecoPicksTopN(ctx context.Context, limit int) (json.RawMessage, error) {
	return ecoFetchJSON(ctx, fmt.Sprintf("%s/api/picks?limit=%d", ecoPicksURL(), limit), nil)
}

// --- wallet_intel.db (read-only) -----------------------------------------

func ecoOpenIntel() (*sql.DB, error) {
	// mode=ro: the gateway must never write to the shared intelligence DB.
	return sql.Open("sqlite3", "file:"+ecoWalletIntelDB()+"?mode=ro&_busy_timeout=2000")
}

// ecoWalletIntelStats aggregates wallet_intel.db into a compact JSON blob.
func ecoWalletIntelStats(ctx context.Context) (json.RawMessage, error) {
	db, err := ecoOpenIntel()
	if err != nil {
		return nil, err
	}
	defer db.Close()

	var wallets, tokens, links, seen int64
	if err := db.QueryRowContext(ctx,
		`SELECT (SELECT COUNT(*) FROM wallets),
		        (SELECT COUNT(*) FROM token_stats),
		        (SELECT COUNT(*) FROM wallet_tokens),
		        (SELECT COUNT(*) FROM seen)`).
		Scan(&wallets, &tokens, &links, &seen); err != nil {
		return nil, fmt.Errorf("wallet_intel counts: %w", err)
	}

	topTokens, err := ecoQueryRows(ctx, db,
		`SELECT symbol, mint, distinct_wallets, buy_volume, first_buy_ts
		 FROM token_stats ORDER BY buy_volume DESC LIMIT 5`,
		[]string{"symbol", "mint", "distinct_wallets", "buy_volume", "first_buy_ts"})
	if err != nil {
		return nil, err
	}
	topWallets, err := ecoQueryRows(ctx, db,
		`SELECT address, tags, buys, sells, volume_usd, edge, last_seen
		 FROM wallets ORDER BY volume_usd DESC LIMIT 5`,
		[]string{"address", "tags", "buys", "sells", "volume_usd", "edge", "last_seen"})
	if err != nil {
		return nil, err
	}

	return json.Marshal(map[string]any{
		"stats": map[string]any{
			"wallets":            wallets,
			"tokens":             tokens,
			"wallet_token_links": links,
			"seen_txs":           seen,
		},
		"top_tokens":  topTokens,
		"top_wallets": topWallets,
	})
}

// ecoWalletIntelToken returns the token_stats row, its tracked holders, and
// the symbol for a mint. token/holders are null/[] when the mint is unknown.
func ecoWalletIntelToken(ctx context.Context, db *sql.DB, mint string) (
	symbol string, token json.RawMessage, holders json.RawMessage, err error) {

	var sym string
	var distinctWallets, buyVolume any
	var firstBuyTS, firstBuyers any
	err = db.QueryRowContext(ctx,
		`SELECT symbol, distinct_wallets, buy_volume, first_buy_ts, first_buyers
		 FROM token_stats WHERE mint = ? COLLATE NOCASE`, mint).
		Scan(&sym, &distinctWallets, &buyVolume, &firstBuyTS, &firstBuyers)
	switch {
	case err == sql.ErrNoRows:
		return "", json.RawMessage("null"), json.RawMessage("[]"), nil
	case err != nil:
		return "", nil, nil, fmt.Errorf("wallet_intel token lookup: %w", err)
	}
	symbol = sym

	token, err = json.Marshal(map[string]any{
		"mint":             mint,
		"symbol":           sym,
		"distinct_wallets": distinctWallets,
		"buy_volume":       buyVolume,
		"first_buy_ts":     firstBuyTS,
		"first_buyers":     firstBuyers,
	})
	if err != nil {
		return "", nil, nil, err
	}

	holderRows, err := ecoQueryRows(ctx, db,
		`SELECT w.address, w.tags, w.buys, w.sells, w.volume_usd, w.edge, w.last_seen, wt.first_buy_ts
		 FROM wallet_tokens wt JOIN wallets w ON w.address = wt.wallet
		 WHERE wt.mint = ? COLLATE NOCASE
		 ORDER BY w.volume_usd DESC LIMIT 20`,
		[]string{"address", "tags", "buys", "sells", "volume_usd", "edge", "last_seen", "first_buy_ts"},
		mint)
	if err != nil {
		return "", nil, nil, err
	}
	holders, err = json.Marshal(holderRows)
	if err != nil {
		return "", nil, nil, err
	}
	return symbol, token, holders, nil
}

// ecoResolveMint maps a symbol to a mint when only the ticker is known.
func ecoResolveMint(ctx context.Context, db *sql.DB, symbol string) (string, error) {
	var mint string
	err := db.QueryRowContext(ctx,
		`SELECT mint FROM token_stats WHERE symbol = ? COLLATE NOCASE LIMIT 1`, symbol).
		Scan(&mint)
	if err == sql.ErrNoRows {
		return "", nil
	}
	return mint, err
}

// ecoQueryRows runs a read-only query and renders each row as a JSON object
// keyed by cols. NULL cells become JSON null.
func ecoQueryRows(ctx context.Context, db *sql.DB, query string, cols []string, args ...any) ([]map[string]any, error) {
	rows, err := db.QueryContext(ctx, query, args...)
	if err != nil {
		return nil, fmt.Errorf("wallet_intel query: %w", err)
	}
	defer rows.Close()

	out := make([]map[string]any, 0, 8)
	for rows.Next() {
		cells := make([]any, len(cols))
		ptrs := make([]any, len(cols))
		for i := range cells {
			ptrs[i] = &cells[i]
		}
		if err := rows.Scan(ptrs...); err != nil {
			return nil, fmt.Errorf("wallet_intel scan: %w", err)
		}
		row := make(map[string]any, len(cols))
		for i, c := range cols {
			row[c] = cells[i]
		}
		out = append(out, row)
	}
	return out, rows.Err()
}

// --- handlers ------------------------------------------------------------

func writeEcoJSON(w http.ResponseWriter, code int, payload any) {
	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(code)
	json.NewEncoder(w).Encode(payload) //nolint:errcheck
}

// ecosystemHandler aggregates the whole playground in one call.
func ecosystemHandler(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodGet {
		w.WriteHeader(http.StatusMethodNotAllowed)
		return
	}
	ctx, cancel := context.WithTimeout(r.Context(), 15*time.Second)
	defer cancel()

	resp := map[string]any{
		"service": ecoSvcName,
		"ts":      time.Now().UTC().Format(time.RFC3339),
	}
	errs := map[string]string{}
	setSection := func(name string, fn func(context.Context) (json.RawMessage, error)) {
		raw, err := fn(ctx)
		if err != nil {
			errs[name] = err.Error()
			resp[name] = json.RawMessage("null")
			return
		}
		resp[name] = raw
	}

	setSection("council_overview", func(c context.Context) (json.RawMessage, error) {
		return ecoFetchJSON(c, ecoVantageURL()+"/api/council/overview", ecoCouncilHeaders())
	})
	setSection("council_verdicts", func(c context.Context) (json.RawMessage, error) {
		return ecoCouncilVerdicts(c, ecoVerdictsTop)
	})
	setSection("council_calibration", func(c context.Context) (json.RawMessage, error) {
		return ecoFetchJSON(c, ecoVantageURL()+"/api/council/calibration", ecoCouncilHeaders())
	})
	setSection("picks", func(c context.Context) (json.RawMessage, error) {
		return ecoPicksTopN(c, ecoPicksTop)
	})
	setSection("pool_health", func(c context.Context) (json.RawMessage, error) {
		return ecoFetchJSON(c, ecoPoolHealthURL()+"/api/poolhealth", nil)
	})
	setSection("wallet_intel", ecoWalletIntelStats)

	if len(errs) > 0 {
		resp["source_errors"] = errs
	}
	writeEcoJSON(w, http.StatusOK, resp)
}

func ecoValidAddress(addr string) bool {
	if len(addr) < 28 || len(addr) > 50 {
		return false
	}
	for _, c := range addr {
		if !(c >= 'a' && c <= 'z' || c >= 'A' && c <= 'Z' || c >= '0' && c <= '9') {
			return false
		}
	}
	return true
}

// tokenIntakeHandler — the "ask Omokoda about token X" agent intake.
// GET /v1/token/{address}[?symbol=TICKER]
func tokenIntakeHandler(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodGet {
		w.WriteHeader(http.StatusMethodNotAllowed)
		return
	}
	addr := strings.TrimSpace(strings.TrimPrefix(r.URL.Path, "/v1/token/"))
	if !ecoValidAddress(addr) {
		writeEcoJSON(w, http.StatusBadRequest, map[string]any{
			"error":   "invalid token address",
			"service": ecoSvcName,
		})
		return
	}

	ctx, cancel := context.WithTimeout(r.Context(), 15*time.Second)
	defer cancel()

	resp := map[string]any{
		"service": ecoSvcName,
		"ts":      time.Now().UTC().Format(time.RFC3339),
		"query":   map[string]any{"address": addr},
	}
	errs := map[string]string{}

	db, dbErr := ecoOpenIntel()
	if dbErr != nil {
		errs["wallet_intel"] = dbErr.Error()
	} else {
		defer db.Close()
	}

	// 1. wallet_intel: the mint row is the ground truth for symbol + activity.
	var symbol string
	var tokenRow json.RawMessage = json.RawMessage("null")
	var holders json.RawMessage = json.RawMessage("[]")
	if db != nil {
		var err error
		symbol, tokenRow, holders, err = ecoWalletIntelToken(ctx, db, addr)
		if err != nil {
			errs["wallet_intel"] = err.Error()
		}
	}
	resp["wallet_intel"] = map[string]any{"token": tokenRow, "holders": holders}

	// 2. optional symbol fallback / override for the council lookup.
	if symbolQ := strings.TrimSpace(r.URL.Query().Get("symbol")); symbolQ != "" {
		symbol = strings.ToUpper(symbolQ)
		if dbErr == nil {
			if mint, err := ecoResolveMint(ctx, db, symbolQ); err != nil {
				errs["wallet_intel"] = err.Error()
			} else if mint != "" {
				resp["query"].(map[string]any)["resolved_mint"] = mint
			}
		}
	}
	resp["query"].(map[string]any)["symbol"] = symbol

	// 3. picks: is this token in the fusion pool?
	pick, err := ecoPicksByAddress(ctx, addr)
	if err != nil {
		errs["picks"] = err.Error()
	}
	resp["picks_score"] = pick // null when absent

	// 4. council verdicts for the token's symbol.
	verdicts, err := ecoVerdictsForSymbol(ctx, symbol, ecoIntakeVerdicts)
	if err != nil {
		errs["council"] = err.Error()
	}
	resp["council_verdicts"] = verdicts

	// 5. pool health snapshot (context for fillability / key pressure).
	poolHealth, err := ecoFetchJSON(ctx, ecoPoolHealthURL()+"/api/poolhealth", nil)
	if err != nil {
		errs["pool_health"] = err.Error()
	} else {
		resp["pool_health"] = poolHealth
	}

	resp["found"] = pick != nil || (string(tokenRow) != "null") || (string(verdicts) != "[]")
	if len(errs) > 0 {
		resp["source_errors"] = errs
	}
	writeEcoJSON(w, http.StatusOK, resp)
}

// ecosystemUIHandler serves the minimal human-readable page over /ecosystem.
func ecosystemUIHandler(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodGet {
		w.WriteHeader(http.StatusMethodNotAllowed)
		return
	}
	w.Header().Set("Content-Type", "text/html; charset=utf-8")
	io.WriteString(w, ecoUIHTML) //nolint:errcheck
}
