;; omokoda-hermetic/src/scoring/ladder.clj
;; Ladder Multipliers — Wisdom Layer (Clojure)
;; Hermetic principle evaluation, policy decisions, scoring logic

(ns omokoda-hermetic.scoring.ladder
  (:require [clojure.spec.alpha :as s]
            [clojure.math.numeric-tower :as math]))

;; ─── Constants (from TOC_CONSTANTS.toml) ───
(def bonus-ladder
  {"gpu_compute"       1.0
   "simulation"        1.0
   "print_job"         2.0
   "sci_sim"           2.0
   "spatial_capture"   2.0
   "aerial_flight"     3.0
   "ground_robot"      3.0
   "sim_to_real"       5.0})

(def bonus-ladder-max
  {"sim_to_real" 10.0})

(def tier-requirements
  {"sim_to_real"   2  ;; T2+
   "aerial_flight" 3  ;; T3+
   "ground_robot"  3}) ;; T3+

(def trust-tiers
  {:T0 0  ;; Unverified
   :T1 1  ;; Basic verification
   :T2 2  ;; Verified work history
   :T3 3  ;; High reputation
   :T4 4}) ;; Council/elite

(def tier-multipliers [1.0 1.5 2.0 3.0 5.0])

;; ─── Specs ───
(s/def ::domain string?)
(s/def ::tier keyword?)
(s/def ::multiplier double?)
(s/def ::proof-value double?)

;; ─── Core Functions ───

(defn tier-multiplier
  "Get tier multiplier for trust tier"
  [tier]
  (get tier-multipliers (trust-tiers tier) 1.0))

(defn tier-can-earn?
  "Check if tier can earn multiplier for domain"
  [tier required-tier]
  (>= (trust-tiers tier) required-tier))

(defn base-multiplier
  "Get base multiplier for work domain"
  [domain]
  (get bonus-ladder domain 1.0))

(defn max-multiplier
  "Get max multiplier for work domain (governance adjustable)"
  [domain]
  (get bonus-ladder-max domain (base-multiplier domain)))

(defn required-tier
  "Get required tier for work domain"
  [domain]
  (get tier-requirements domain 0))

(defn effective-multiplier
  "Compute effective multiplier for agent tier and domain"
  [tier domain]
  (let [base (base-multiplier domain)
        max-mult (max-multiplier domain)
        required (required-tier domain)
        tier-level (trust-tiers tier)]
    (if (>= tier-level required)
      (min base max-mult)
      1.0)))  ;; Tier gate fails → base only

;; ─── Proof Value Validation ───

(defn validate-proof-factors
  "Validate proof factors from verified evidence (not request args)
  
  Returns map with :valid? and :factors or :error"
  [{:keys [difficulty quality novelty verification independence utility] :as factors}]
  (let [clamp #(min 1.0 (max 0.0 %))
        clamped (into {} (for [[k v] factors] [k (clamp v)]))
        proof-value (reduce * (vals clamped))]
    (if (>= proof-value 0.3)
      {:valid? true :factors clamped :proof-value proof-value}
      {:valid? false :factors clamped :proof-value proof-value
       :error "Proof value below threshold (0.3)"})))

;; ─── Reward Computation (SINGLE FORMULA) ───

(defn compute-reward
  "Single reward formula - replaces two disagreeing formulas in OSOVM
  
  Reward = verified_gpu_hours × ladder_multiplier(domain) × proof_value
  Where proof_value ≥ threshold (hard gate)
  
  NO caller-supplied quantity or multiplier."
  [gpu-hours domain proof-value tier]
  {:pre [(>= gpu-hours 0)
         (>= proof-value 0)
         (<= proof-value 1)
         (contains? trust-tiers tier)]}
  
  (if (< proof-value 0.3)
    {:synapse 0.0 :dopamine-equiv 0.0 :error "Proof value below threshold"}
    (let [mult (effective-multiplier tier domain)
          synapse (* gpu-hours 1000.0 mult proof-value)
          dopamine-equiv (* gpu-hours 1000.0 proof-value)]  ;; No ladder for DOP
      {:synapse synapse
       :dopamine-equiv dopamine-equiv
       :multiplier mult
       :proof-value proof-value})))

;; ─── Anti-Gaming Caps ───

(def per-agent-epoch-cap 50000000)  ;; 50M DOPAMINE per epoch
(def repeat-limit 3)                ;; Same model_hash ≤ 3 times/epoch

(defn check-epoch-cap
  "Check if agent can earn more DOPAMINE this epoch"
  [agent-npub requested-dopamine epoch-tracker]
  (let [earned (get-in epoch-tracker [agent-npub :dopamine_earned] 0)
        projected (+ earned requested-dopamine)]
    (if (> projected per-agent-epoch-cap)
      {:allowed? false
       :reason (str "Per-agent epoch cap exceeded: " projected "/" per-agent-epoch-cap)
       :current earned :cap per-agent-epoch-cap}
      {:allowed? true :remaining (- per-agent-epoch-cap projected)})))

(defn check-repeat-limit
  "Check repeat limit for model/sim hash"
  [agent-npub model-hash repeat-tracker]
  (let [count (get-in repeat-tracker [agent-npub model-hash] 0)]
    (if (>= count repeat-limit)
      {:allowed? false
       :reason (str "Repeat limit exceeded for model " model-hash ": " count "/" repeat-limit)
       :current count :cap repeat-limit}
      {:allowed? true :remaining (- repeat-limit count)})))

;; ─── Synapse → Dopamine Conversion ───

(def synapse-to-dopamine-factor 0.85)  ;; Efficiency tax
(def conversion-ratio 0.1)              ;; 10 DOP → 1 SYN

(defn synapse->dopamine
  "Convert SYNAPSE to DOPAMINE with 0.85 efficiency tax"
  [synapse]
  (* synapse (/ 1.0 conversion-ratio) synapse-to-dopamine-factor))
  ;; = synapse * 10 * 0.85 = synapse * 8.5

;; ─── Per-Agent Capacity Derivation ───

(defn derive-capacity
  "per-agent capacity (GPU-h) = (contributed GPU-hours × tier_weight) / active_agent_count"
  [contributed-hours tier active-agent-count]
  {:pre [(>= contributed-hours 0) (pos? active-agent-count)]}
  (let [tier-weight (tier-multiplier tier)]
    (/ (* contributed-hours tier-weight) active-agent-count)))

;; ─── Dynamic Pricing ───

(defn synapse-price
  "price_h = base_ase_per_hour × (1 + U × spread)
   U = allocated / total
   Guards: per-epoch clamp ±0.002, TWAP over Koodu epoch"
  [base-ase-per-hour allocated total & {:keys [spread] :or {spread 1.0}}]
  {:pre [(>= allocated 0) (pos? total)]}
  (let [U (/ allocated total)
        price (* base-ase-per-hour (+ 1 (* U spread)))]
    ;; Per-epoch clamp would be applied by caller with TWAP
    price))

(defn dopamine-price
  "price_dop = base_synapse_per_dopamine × (1 + D × spread)
   D = agents_demanding / agents_with_capacity"
  [base-synapse-per-dopamine agents-demanding agents-with-capacity & {:keys [spread] :or {spread 1.0}}]
  {:pre [(>= agents-demanding 0) (pos? agents-with-capacity)]}
  (let [D (/ agents-demanding agents-with-capacity)
        price (* base-synapse-per-dopamine (+ 1 (* D spread)))]
    price))

;; ─── Governance-Adjustable Parameters ───

(def ^:dynamic *pool-ratios*
  {"veilsim"    0.20
   "rnd"        0.15
   "governance" 0.15
   "reserve"    0.15
   "compute"    0.15
   "storage"    0.10
   "witness"    0.05
   "treasury"   0.05})

(defn update-pool-ratios!
  "Governance can adjust pool ratios via proposal + Bínò sign-off"
  [new-ratios]
  (let [total (reduce + (vals new-ratios))]
    (when (Math/abs (- total 1.0) 1e-10)
      (throw (ex-info "Pool ratios must sum to 1.0" {:total total})))
    (alter-var-root #'*pool-ratios* (constantly new-ratios))
    *pool-ratios*))

;; ─── Koodu Ritual Multipliers ───

(def koodu-multipliers
  {:sabbath         1.10   ;; Every 7 days
   :eshu-squared    1.369  ;; 7² = 49th day (crossroads gate)
   :jubilee-minor   2.00   ;; 49th day, 7×7 completion
   :capstone        1.50   ;; Completion gate
   :void            0.00}) ;; Rest/no-op gate

(defn apply-koodu-multiplier
  "Apply Koodu ritual multiplier based on current day"
  [base-value day-of-cycle]
  (cond
    (= (mod day-of-cycle 7) 0) (* base-value (:sabbath koodu-multipliers))
    (= (mod day-of-cycle 49) 0) (* base-value (:jubilee-minor koodu-multipliers))
    (= (mod day-of-cycle 7) 0) (* base-value (:eshu-squared koodu-multipliers))
    :else base-value))

;; ─── Export ───
(defn- public-fns []
  '[tier-multiplier tier-can-earn? base-multiplier max-multiplier required-tier
    effective-multiplier validate-proof-factors compute-reward
    check-epoch-cap check-repeat-limit synapse->dopamine
    derive-capacity synapse-price dopamine-price
    update-pool-ratios! apply-koodu-multiplier])

(comment
  ;; Test examples
  (effective-multiplier :T2 "sim_to_real")    ;; => 5.0
  (effective-multiplier :T1 "sim_to_real")    ;; => 1.0 (tier gate fails)
  (effective-multiplier :T3 "aerial_flight")  ;; => 3.0
  (compute-reward 1.0 "sim_to_real" 0.8 :T2)  ;; => {:synapse 4000.0 ...}
  (validate-proof-factors {:difficulty 1.0 :quality 0.8 :novelty 1.0 
                           :verification 1.0 :independence 0.8 :utility 0.92})
  ;; => {:valid? true, :proof-value 0.589...}
  )