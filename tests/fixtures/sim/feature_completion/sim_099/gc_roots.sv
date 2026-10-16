// SIM-099 step 2: objects reachable only through state that DPI, VPI and
// assertions hold across a collection. Run under LLG_GC_STRESS=1
// LLG_GC_VERIFY=1, a missed root ends the run with "access to a reclaimed
// class object"; the output must equal the run without stress.
`timescale 1ns / 1ns

class node_c;
  int v;
  node_c peer;
  function new(int x);
    v = x;
  endfunction
endclass

module tb;
  import "DPI-C" context function int c_churn(input int n);
  export "DPI-C" function sv_alloc;

  node_c kept[$];
  logic [7:0] req = 8'd0;
  logic [7:0] ack = 8'd0;

  // Called from C inside c_churn: garbage cycles, one kept object.
  function automatic int sv_alloc(input int k);
    node_c g = new(k);
    g.peer = g;
    if (k == 0) kept.push_back(g);
    return k;
  endfunction

  // A deferred assertion's action receives its by-value arguments as they
  // were when the assertion was evaluated (IEEE 1800-2009 16.4).
  function automatic void report(node_c it);
    $display("[%0t] deferred action sees %0d %0d", $time, it.v, it.peer.v);
  endfunction

  // Only this activation reaches `mine` while C allocates through the
  // export and while the task waits afterwards.
  task automatic across_dpi(int v);
    node_c mine = new(v);
    int made;
    mine.peer = new(v + 1);
    made = c_churn(200);
    #1;
    $display("[%0t] after DPI churn %0d: mine %0d %0d kept %0d", $time, made, mine.v, mine.peer.v,
             kept.size());
  endtask

  // gc_roots.c answers each change of req with ack = req + 1 one time unit
  // later (vpi_put_value with an inertial delay) from a value-change
  // callback; `mine` is reachable only from this waiting activation.
  task automatic across_vpi(int v);
    node_c mine = new(v);
    mine.peer = mine;
    req = 8'(v);
    @(ack);
    $display("[%0t] after VPI answer ack=%0d: mine %0d %0d", $time, ack, mine.v, mine.peer.v);
  endtask

  initial begin
    node_c h = new(7);
    h.peer = new(8);
    h.peer.peer = h;
    assert #0 (h.v == 0)
    else report(h);
    h = null;
    #1;
    across_dpi(20);
    across_vpi(30);
    across_vpi(40);
    $finish;
  end
endmodule
