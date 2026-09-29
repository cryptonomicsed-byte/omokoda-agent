defmodule OmokodaSwarm.Wave9Test do
  use ExUnit.Case, async: false

  setup do
    Application.ensure_started(:omokoda_swarm)
    Process.sleep(200)
    :ok
  end

  # ── Backend Behaviour ──────────────────────────────────────────────────────

  describe "LocalBackend" do
    test "name/0 returns :local" do
      assert OmokodaSwarm.Backends.LocalBackend.name() == :local
    end

    test "available?/0 is always true" do
      assert OmokodaSwarm.Backends.LocalBackend.available?()
    end

    test "execute/2 runs a fun task and returns {:ok, result}" do
      task = %{fun: fn -> 42 end}
      assert {:ok, 42} = OmokodaSwarm.Backends.LocalBackend.execute(task, [])
    end

    test "execute/2 passes through non-fun tasks" do
      assert {:ok, :hello} = OmokodaSwarm.Backends.LocalBackend.execute(:hello, [])
    end

    test "execute/2 returns {:error, :timeout} on timeout" do
      task = %{fun: fn -> Process.sleep(5_000) end}
      assert {:error, :timeout} = OmokodaSwarm.Backends.LocalBackend.execute(task, timeout: 50)
    end
  end

  describe "RemoteBackend" do
    test "name/0 returns :remote" do
      assert OmokodaSwarm.Backends.RemoteBackend.name() == :remote
    end

    test "available?/0 returns false when no nodes connected" do
      assert OmokodaSwarm.Backends.RemoteBackend.available?() == false
    end

    test "execute/2 returns {:error, :no_remote_nodes} when no peers" do
      assert {:error, :no_remote_nodes} =
               OmokodaSwarm.Backends.RemoteBackend.execute(:task, [])
    end
  end

  describe "ContainerBackend" do
    test "name/0 returns :container" do
      assert OmokodaSwarm.Backends.ContainerBackend.name() == :container
    end

    test "available?/0 returns a boolean" do
      result = OmokodaSwarm.Backends.ContainerBackend.available?()
      assert is_boolean(result)
    end
  end

  # ── BackendRegistry ────────────────────────────────────────────────────────

  describe "BackendRegistry" do
    setup do
      {:ok, pid} = OmokodaSwarm.BackendRegistry.start_link(name: :test_backend_registry)
      on_exit(fn -> if Process.alive?(pid), do: GenServer.stop(pid) end)
      %{registry: pid}
    end

    test "list/1 returns all three default backends", %{registry: r} do
      entries = OmokodaSwarm.BackendRegistry.list(r)
      names = Enum.map(entries, & &1.name) |> Enum.sort()
      assert names == [:container, :local, :remote]
    end

    test "get/2 returns the backend module", %{registry: r} do
      assert OmokodaSwarm.BackendRegistry.get(r, :local) ==
               OmokodaSwarm.Backends.LocalBackend
    end

    test "select/2 returns an available backend", %{registry: r} do
      result = OmokodaSwarm.BackendRegistry.select(r)
      assert result != nil
      assert result.available?()
    end

    test "select/2 with :prefer :local always picks LocalBackend", %{registry: r} do
      result = OmokodaSwarm.BackendRegistry.select(r, prefer: :local)
      assert result == OmokodaSwarm.Backends.LocalBackend
    end

    test "register/2 and unregister/2 round-trip", %{registry: r} do
      :ok = OmokodaSwarm.BackendRegistry.register(r, OmokodaSwarm.Backends.LocalBackend)
      assert OmokodaSwarm.BackendRegistry.get(r, :local) != nil

      :ok = OmokodaSwarm.BackendRegistry.unregister(r, :local)
      assert OmokodaSwarm.BackendRegistry.get(r, :local) == nil
    end
  end

  # ── TeammateMailbox ────────────────────────────────────────────────────────

  describe "TeammateMailbox" do
    setup do
      {:ok, pid} = OmokodaSwarm.TeammateMailbox.start_link([])
      %{mailbox: pid}
    end

    test "push and pop preserve FIFO order", %{mailbox: m} do
      OmokodaSwarm.TeammateMailbox.push(m, :first)
      OmokodaSwarm.TeammateMailbox.push(m, :second)

      assert {:ok, :first} = OmokodaSwarm.TeammateMailbox.pop(m)
      assert {:ok, :second} = OmokodaSwarm.TeammateMailbox.pop(m)
      assert {:error, :empty} = OmokodaSwarm.TeammateMailbox.pop(m)
    end

    test "peek does not remove the item", %{mailbox: m} do
      OmokodaSwarm.TeammateMailbox.push(m, :msg)
      assert {:ok, :msg} = OmokodaSwarm.TeammateMailbox.peek(m)
      assert {:ok, :msg} = OmokodaSwarm.TeammateMailbox.peek(m)
    end

    test "size reflects queue length", %{mailbox: m} do
      assert 0 = OmokodaSwarm.TeammateMailbox.size(m)
      OmokodaSwarm.TeammateMailbox.push(m, :a)
      OmokodaSwarm.TeammateMailbox.push(m, :b)
      assert 2 = OmokodaSwarm.TeammateMailbox.size(m)
    end

    test "drain returns all items and empties queue", %{mailbox: m} do
      OmokodaSwarm.TeammateMailbox.push(m, 1)
      OmokodaSwarm.TeammateMailbox.push(m, 2)
      OmokodaSwarm.TeammateMailbox.push(m, 3)

      assert [1, 2, 3] = OmokodaSwarm.TeammateMailbox.drain(m)
      assert 0 = OmokodaSwarm.TeammateMailbox.size(m)
    end
  end

  # ── TeammateContext ────────────────────────────────────────────────────────

  describe "TeammateContext" do
    setup do
      {:ok, pid} = OmokodaSwarm.TeammateContext.start_link([])
      %{ctx: pid}
    end

    test "put/get/delete round-trip", %{ctx: c} do
      :ok = OmokodaSwarm.TeammateContext.put(c, :model, :sonnet)
      assert :sonnet = OmokodaSwarm.TeammateContext.get(c, :model)
      :ok = OmokodaSwarm.TeammateContext.delete(c, :model)
      assert nil == OmokodaSwarm.TeammateContext.get(c, :model)
    end

    test "get with default", %{ctx: c} do
      assert :default = OmokodaSwarm.TeammateContext.get(c, :missing, :default)
    end

    test "keys/1 returns all keys", %{ctx: c} do
      OmokodaSwarm.TeammateContext.put(c, :a, 1)
      OmokodaSwarm.TeammateContext.put(c, :b, 2)
      assert [:a, :b] == OmokodaSwarm.TeammateContext.keys(c) |> Enum.sort()
    end

    test "merge/2 merges a map in", %{ctx: c} do
      OmokodaSwarm.TeammateContext.put(c, :x, 1)
      OmokodaSwarm.TeammateContext.merge(c, %{y: 2, z: 3})
      assert %{x: 1, y: 2, z: 3} = OmokodaSwarm.TeammateContext.to_map(c)
    end
  end

  # ── Teammate ───────────────────────────────────────────────────────────────

  describe "Teammate" do
    setup do
      id = "test-teammate-#{System.unique_integer([:positive])}"
      {:ok, _pid} = OmokodaSwarm.Teammate.start_link(id: id, model: :haiku)
      on_exit(fn ->
        try do
          OmokodaSwarm.Teammate.stop(id)
        catch
          :exit, _ -> :ok
        end
      end)
      %{id: id}
    end

    test "get_state returns initial info", %{id: id} do
      assert {:ok, info} = OmokodaSwarm.Teammate.get_state(id)
      assert info.state == :idle
      assert info.model == :haiku
      assert info.mailbox_size == 0
    end

    test "send_message transitions to :running and enqueues", %{id: id} do
      :ok = OmokodaSwarm.Teammate.send_message(id, "hello")
      assert {:ok, info} = OmokodaSwarm.Teammate.get_state(id)
      assert info.state == :running
      assert info.mailbox_size == 1
    end

    test "set_model changes the model", %{id: id} do
      :ok = OmokodaSwarm.Teammate.set_model(id, :opus)
      assert {:ok, info} = OmokodaSwarm.Teammate.get_state(id)
      assert info.model == :opus
    end

    test "valid transitions succeed", %{id: id} do
      :ok = OmokodaSwarm.Teammate.transition(id, :running)
      :ok = OmokodaSwarm.Teammate.transition(id, :done)
      :ok = OmokodaSwarm.Teammate.transition(id, :idle)
    end

    test "invalid transition returns error", %{id: id} do
      assert {:error, {:invalid_transition, :idle, :done}} =
               OmokodaSwarm.Teammate.transition(id, :done)
    end
  end

  # ── TeammateLayoutManager ──────────────────────────────────────────────────

  describe "TeammateLayoutManager" do
    setup do
      {:ok, pid} = OmokodaSwarm.TeammateLayoutManager.start_link(name: :test_layout)
      on_exit(fn -> if Process.alive?(pid), do: GenServer.stop(pid) end)
      %{mgr: pid}
    end

    test "add and list teammates", %{mgr: m} do
      :ok = OmokodaSwarm.TeammateLayoutManager.add_teammate(m, "alpha", role: :lead)
      :ok = OmokodaSwarm.TeammateLayoutManager.add_teammate(m, "beta", role: :worker)

      assert ["alpha", "beta"] =
               OmokodaSwarm.TeammateLayoutManager.list_teammates(m) |> Enum.sort()
    end

    test "get_layout returns entries sorted by position", %{mgr: m} do
      :ok = OmokodaSwarm.TeammateLayoutManager.add_teammate(m, "first")
      :ok = OmokodaSwarm.TeammateLayoutManager.add_teammate(m, "second")

      [e1, e2] = OmokodaSwarm.TeammateLayoutManager.get_layout(m)
      assert e1.position < e2.position
    end

    test "add duplicate returns error", %{mgr: m} do
      :ok = OmokodaSwarm.TeammateLayoutManager.add_teammate(m, "dup")
      assert {:error, :already_exists} = OmokodaSwarm.TeammateLayoutManager.add_teammate(m, "dup")
    end

    test "remove_teammate removes entry", %{mgr: m} do
      :ok = OmokodaSwarm.TeammateLayoutManager.add_teammate(m, "gone")
      :ok = OmokodaSwarm.TeammateLayoutManager.remove_teammate(m, "gone")
      assert [] = OmokodaSwarm.TeammateLayoutManager.list_teammates(m)
    end

    test "list_by_role filters correctly", %{mgr: m} do
      :ok = OmokodaSwarm.TeammateLayoutManager.add_teammate(m, "lead1", role: :lead)
      :ok = OmokodaSwarm.TeammateLayoutManager.add_teammate(m, "work1", role: :worker)

      leads = OmokodaSwarm.TeammateLayoutManager.list_by_role(m, :lead)
      assert length(leads) == 1
      assert hd(leads).id == "lead1"
    end

    test "reorder updates position", %{mgr: m} do
      :ok = OmokodaSwarm.TeammateLayoutManager.add_teammate(m, "mover")
      :ok = OmokodaSwarm.TeammateLayoutManager.reorder(m, "mover", 99)

      [entry] = OmokodaSwarm.TeammateLayoutManager.get_layout(m)
      assert entry.position == 99
    end
  end

  # ── PermissionSync ─────────────────────────────────────────────────────────

  describe "PermissionSync" do
    setup do
      {:ok, pid} = OmokodaSwarm.PermissionSync.start_link(name: :test_perm_sync)
      on_exit(fn -> if Process.alive?(pid), do: GenServer.stop(pid) end)
      %{sync: pid}
    end

    test "request returns a request ID", %{sync: s} do
      assert {:ok, req_id} = OmokodaSwarm.PermissionSync.request(s, "agent1", "bash", 1)
      assert is_binary(req_id)
    end

    test "approve grants permission and check returns true", %{sync: s} do
      {:ok, req_id} = OmokodaSwarm.PermissionSync.request(s, "agent1", "read", 0)
      {:ok, _grant} = OmokodaSwarm.PermissionSync.approve(s, req_id, "leader")

      assert OmokodaSwarm.PermissionSync.check("agent1", "read", 0)
    end

    test "check returns false for unknown grantee" do
      refute OmokodaSwarm.PermissionSync.check("nobody", "bash", 0)
    end

    test "deny updates request status", %{sync: s} do
      {:ok, req_id} = OmokodaSwarm.PermissionSync.request(s, "agent2", "write", 2)
      :ok = OmokodaSwarm.PermissionSync.deny(s, req_id, "too risky")

      requests = OmokodaSwarm.PermissionSync.list_requests(s)
      denied = Enum.find(requests, &(&1.id == req_id))
      assert denied.status == :denied
      assert denied.reason == "too risky"
    end

    test "revoke removes grant", %{sync: s} do
      {:ok, req_id} = OmokodaSwarm.PermissionSync.request(s, "agent3", "net", 0)
      {:ok, grant} = OmokodaSwarm.PermissionSync.approve(s, req_id, "leader")

      assert OmokodaSwarm.PermissionSync.check("agent3", "net", 0)

      :ok = OmokodaSwarm.PermissionSync.revoke(s, grant.id)
      refute OmokodaSwarm.PermissionSync.check("agent3", "net", 0)
    end

    test "list_grants returns current grant set", %{sync: s} do
      before = length(OmokodaSwarm.PermissionSync.list_grants())
      {:ok, req_id} = OmokodaSwarm.PermissionSync.request(s, "agent4", "tool", 0)
      OmokodaSwarm.PermissionSync.approve(s, req_id, "leader")
      assert length(OmokodaSwarm.PermissionSync.list_grants()) == before + 1
    end

    test "approve with ttl_secs stores an expiry timestamp", %{sync: s} do
      {:ok, req_id} = OmokodaSwarm.PermissionSync.request(s, "agent5", "exec", 0)
      {:ok, grant} = OmokodaSwarm.PermissionSync.approve(s, req_id, "leader", ttl_secs: 3600)
      assert is_integer(grant.expires_at)
      assert grant.expires_at > System.system_time(:second)
    end

    test "approve not_found returns error", %{sync: s} do
      assert {:error, :not_found} = OmokodaSwarm.PermissionSync.approve(s, "bad-id", "leader")
    end

    test "deny not_found returns error", %{sync: s} do
      assert {:error, :not_found} = OmokodaSwarm.PermissionSync.deny(s, "bad-id", "reason")
    end
  end
end
