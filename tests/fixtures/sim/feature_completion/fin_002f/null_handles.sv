// Null handles in every operation that takes one.
`timescale 1ns/1ns
interface ifc(input logic clk);
  logic [7:0] d;
  logic [7:0] mem[4];
  event ie;
  int cnt;
  function automatic int f(int x); return x + cnt; endfunction
  task automatic t(input int x); #1 cnt = x; endtask
  clocking cb @(posedge clk); input d; endclocking
  modport mp(input d, import f);
endinterface
module tb;
  class node;
    int v = 7;
    int arr[3];
    int q[$];
    string s;
    event ev;
    mailbox #(int) mb;
    node next;
    rand int r;
    covergroup emb; coverpoint v; endgroup
    function new(); emb = new; endfunction
    static int st = 5;
    constraint c { r inside {[1:3]}; }
    function int get(); return v; endfunction
    virtual function int vget(); return v; endfunction
    static function int sget(); return st; endfunction
    virtual task tsk(); #1 v++; endtask
    function void set(int x); v = x; endfunction
  endclass
  class dnode extends node;
    virtual function int vget(); return v + 1; endfunction
  endclass
  bit clk;
  always #5 clk = ~clk;
  ifc i0(clk);
  virtual ifc vif, vif2;
  virtual ifc.mp vmp;
  mailbox mb0;
  mailbox #(int) mb;
  semaphore sem;
  process p;
  event e, ev2;
  node n, m;
  dnode d;
  int x, ctx, k;
  string s;
  int q[$];
  chandle ch;
  logic [7:0] nb;
  node arr_n[2];
  node qn[$];
  node an[string];
  covergroup cg with function sample(int a); cp: coverpoint a { bins b[] = {[0:3]}; } endgroup
  cg cgh;
  event en = null, eq[$];
  event em[2];
  function automatic int fnum(); int l = mb.num(); $display("fnum went on"); return l; endfunction
  function automatic void fput(); sem.put(1); $display("fput went on"); endfunction
  task automatic wait_formal(event x); @x; endtask
  task run(int c);
    case (c)
      0: x = n.v;
      1: n.v = 3;
      2: x = n.get();
      3: x = n.vget();
      4: begin x = n.sget(); $display("static %0d", x); end
      5: begin x = node::st; $display("static prop %0d", x); end
      6: n.tsk();
      7: x = n.arr[1];
      8: n.arr[2] = 4;
      9: n.q.push_back(1);
      10: x = n.q.size();
      11: s = n.s;
      12: n.s = "abc";
      13: x = n.next.v;
      14: begin m = new; x = m.next.v; end
      15: begin m = new; m.next.next = m; end
      16: x = n.randomize();
      17: begin void'($cast(d, n)); $display("cast null %0d", d == null); end
      18: begin x = $cast(d, n); $display("cast null ok %0d", x); end
      19: nb <= n.v;
      20: ;
      21: @(n.ev) x = 1;
      22: -> n.ev;
      23: wait (n.v == 3);
      24: $display("%0d", n.v);
      25: $display("handle %p", n);
      26: foreach (n.arr[i]) x += n.arr[i];
      27: foreach (n.q[i]) x += n.q[i];
      28: x = n.mb.num();
      29: begin m = new; x = m.mb.num(); end
      30: begin m = new; m.mb.put(1); end
      31: begin m = new n; $display("copy"); end
      32: x = (n == null) ? 1 : n.v;
      33: x = (n != null && n.v > 0);
      34: case (n.v) 1: x = 1; default: x = 2; endcase
      35: fork x = n.v; join
      36: fork x = n.v; #1 $display("sibling"); join_none
      37: x += n.v;
      38: n.v++;
      39: s = $sformatf("%0d", n.v);
      40: x = arr_n[1].v;
      41: x = qn[0].v;
      42: x = an["none"].v;
      43: arr_n[0].v = 2;
      44: qn[0] = null;
      45: x = vif.d;
      46: vif.d = 8'h5;
      47: vif.d <= 8'h6;
      48: x = vif.f(2);
      49: vif.t(3);
      50: @(posedge vif.clk) x = 1;
      51: @(vif.cb) x = 1;
      52: x = vif.cb.d;
      53: wait (vif.d == 3);
      54: ;
      55: x = vif.mem[1];
      56: vif.mem[2] = 3;
      57: $display("vif %0d %0d", vif == null, vif == vif2);
      58: x = vmp.d;
      59: begin vif = i0; vif.d = 4; vif2 = vif; vif = null; $display("vif2 %0d", vif2.d); x = vif.d; end
      60: mb0.put(1);
      61: x = mb0.num();
      62: mb.get(x);
      63: x = mb.try_get(k);
      64: mb.put(3);
      65: sem.get(1);
      66: sem.put(1);
      67: x = sem.try_get(1);
      68: $display("%s", p.status().name());
      69: p.kill();
      70: p.await();
      71: p.suspend();
      72: p.resume();
      73: begin e = null; -> e; $display("trigger null event"); end
      74: begin e = null; ->> e; $display("nb trigger null event"); end
      75: begin e = null; $display("triggered %0d", e.triggered); end
      76: begin e = null; @e; $display("woke"); end
      77: begin e = null; wait (e.triggered); $display("wait triggered"); end
      78: begin event e1, e2; e1 = null; e2 = null; $display("cmp %0d %0d", e1 == e2, e1 == null); end
      79: cgh.sample(1);
      80: $display("%0.1f", cgh.get_inst_coverage());
      81: $display("%0.1f", cgh.get_coverage());
      82: cgh.stop();
      83: begin n = null; n.set(3); end
      84: assert (n.v == 3) else $display("assert else");
      85: begin m = new; m.next = n; x = m.next.get(); end
      86: $display("chandle %0d", ch == null);
      87: x = n.s.len();
      88: n.arr = '{1, 2, 3};
      89: q = n.q;
      90: {x, k} = {n.v, n.r};
      91: repeat (n.v) x++;
      92: #(n.v) x = 1;
      93: x = #1 n.v;
      94: n.rand_mode(0);
      95: n.c.constraint_mode(0);
      96: x = n.r.rand_mode();
      97: x = q.sum with (n.v);
      98: begin q = '{1, 2}; x = q.sum with (n.v); end
      99: begin q = '{1, 2}; q = q.find with (item == n.v); end
      100: void'(std::randomize(x) with { x == n.v; });
      101: n.srandom(3);
      102: s = n.get_randstate();
      103: wait_order (e, n.ev) x = 1;
      104: ;
      105: disable fork;
      106: @(posedge clk or en);
      107: @(en or ev2);
      108: @(en iff clk);
      109: @(eq[0]);
      110: @(em[k + 5]);
      111: wait (em[k + 5].triggered);
      112: x = @en 1;
      113: x <= @en 1;
      114: ->> @en ev2;
      115: repeat (2) @en;
      116: wait_order (ev2, en) x = 1;
      117: wait_formal(en);
      118: begin m = new; m.ev = null; @(m.ev); end
      119: -> en;
      120: ->> em[k + 5];
      121: x = fnum();
      122: fput();
      123: wait (mb.num() == 0);
      124: if (sem.try_get(1) == 0) $display("no key");
      125: void'(mb.try_put(1));
      126: void'(mb.try_peek(k));
      127: s = {"a", p.get_randstate()};
      128: begin s = n.get_randstate(); n.set_randstate(s); end
      129: $display("%0.1f", n.emb.get_coverage());
      130: $strobe("%0d", mb.num());
      131: x = 1 + (mb.num() > 0 ? 1 : fnum());
      132: begin repeat (2) @en; end
      133: if (em[k + 5].triggered || en.triggered || m == null) $display("false");
      default: $display("none");
    endcase
  endtask
  initial begin
    void'($value$plusargs("ctx=%d", ctx));
    fork begin run(ctx); $display("[%0t] ctx %0d went on x=%0d", $time, ctx, x); end join_none
    #20 $display("[%0t] main ends", $time);
    $finish;
  end
  final $display("[%0t] final", $time);
endmodule
