// An action taken by a function in the operands of many statements.
`timescale 1ns/1ns
module tb;
  string act;
  int armed;
  process victim_p;
  int ctx;
  int after_count;

  function automatic int take(int v);
    if (armed) begin
      armed = 0;
      case (act)
        "kill": begin process p; p = process::self(); p.kill(); end
        "disable": disable tb.victim;
        "finish": $finish;
        "fatal": $fatal(1, "stop here");
        default: ;
      endcase
      if (act != "finish" && act != "fatal") begin
      after_count++;
      end
    end
    return v;
  endfunction

  class node;
    int v = take(3);
    int w;
    function new(int x = take(4));
      w = take(x);
    endfunction
    function int get(); return take(v); endfunction
  endclass

  class rnd;
    rand bit [3:0] a;
    rand bit [3:0] b;
    constraint c { a == b; b inside {[1:4]}; }
    function bit [3:0] lim(bit [3:0] x); return take(x); endfunction
  endclass

  class holder;
    rand rnd kids[2];
    function new(); foreach (kids[i]) kids[i] = new; endfunction
    function void post_randomize(); void'(take(1)); endfunction
  endclass

  covergroup cg with function sample(int x);
    cp: coverpoint x { bins b[] = {[0:3]}; }
  endgroup

  int q[$], dyn[], sorted[$], idx[$];
  int aa[string];
  string s;
  int mem[4];
  logic [99:0] wide;
  struct { int a; string b; int c[$]; } rec;
  int x, y;
  cg cover_i = new;
  mailbox #(int) box = new();
  semaphore sem = new(1);
  node n;
  rnd r1;
  holder h1;
  event e;
  int fd;
  wire logic [7:0] net_v;
  logic [7:0] lhs_a, lhs_b;

  task automatic add(input int a, output int o, inout int io);
    o = a + io; io = io + 1;
  endtask

  task body(int c);
    case (c)
      0: x = take(1) + take(2);
      1: q = '{take(1), 2, 3};
      2: q.push_back(take(7));
      3: begin sorted = '{3, 1, 2}; sorted.sort with (take(item)); end
      4: begin idx = sorted.find_index with (take(item) == 2); end
      5: x = sorted.sum with (take(item));
      6: dyn = new[take(5)];
      7: aa["k"] = take(5);
      8: s = $sformatf("%0d %s", take(1), "abc");
      9: s = {"abc", $sformatf("%0d", take(2))};
      10: $display("never %0d", take(1));
      11: mem[take(2)] = 9;
      12: n = new(take(8));
      13: begin armed = 1; n = new; end
      14: begin r1 = new; x = r1.randomize(); end
      15: begin h1 = new; x = h1.randomize(); end
      16: cover_i.sample(take(2));
      17: box.put(take(3));
      18: sem.put(take(1));
      19: rec = '{take(1), "zz", '{1, take(2)}};
      20: wide = {take(1), 68'h1} << take(3);
      21: #(take(2)) x = 1;
      22: wait (take(1));
      23: repeat (take(2)) x++;
      24: x <= take(4);
      25: x = #1 take(4);
      26: x <= #(take(2)) 4;
      27: case (take(1)) 1: x = 1; default: x = 2; endcase
      28: foreach (sorted[i]) x = take(sorted[i]);
      29: add(take(1), x, y);
      30: add(1, mem[take(1)], y);
      31: if (take(1) inside {[0:take(2)]}) x = 5;
      32: randcase take(1): x = 1; 2: x = 2; endcase
      33: x = take(0) ? 1 : take(2);
      34: assert (take(1)) x = 1; else x = 2;
      35: x = take(6);
      36: for (int i = take(0); i < take(3); i++) x = i;
      37: s = string'(take(65));
      38: {lhs_a, lhs_b} = {8'(take(1)), 8'(take(2))};
      39: fork begin x = take(1); $display("child went on"); end join
      40: fork begin x = take(1); $display("child went on"); end #1; join_any
      41: x = n.get();
      42: begin sorted.delete(); sorted = '{1}; sorted = {sorted, take(1), sorted}; end
      43: q = sorted.find with (item == take(1));
      44: sorted.rsort(it) with (take(it));
      45: begin aa.delete(); aa["x"] = 1; x = aa.sum with (take(item)); end
      46: $swrite(s, "%0d", take(1));
      47: x = $value$plusargs({"act", s.substr(0, take(-1))}, s);
      48: x = take(5);
      49: x = take(6);
      50: x += take(2);
      51: mem[take(1)]++;
      52: x = take(1);
      53: x = take(1);
      54: x = $countones(take(3)) + $clog2(take(8));
      55: s = $sformatf("%s", (take(1) ? "yes" : "no"));
      56: begin rec.c.push_front(take(1)); rec.c.insert(take(0), take(1)); end
      57: wide[take(3) +: 4] = take(5);
      58: randsequence (main) main : one := (take(1)) | two := 2; one : { x = 1; }; two : { x = 2; }; endsequence
      59: randsequence (main) main : one; one : { x = take(1); }; endsequence
      60: do x = take(1); while (take(0));
      61: begin : inner x = take(1); end
      62: wait fork;
      63: x = take(1);
      default: ;
    endcase
  endtask

  initial begin
    void'($value$plusargs("act=%s", act));
    void'($value$plusargs("ctx=%d", ctx));
    if (act == "finish" || act == "fatal") begin
      n = new; armed = 1;
      fork body(ctx); join_none
      #10 $display("[%0t] survived %0d", $time, ctx);
      $finish;
    end
    for (int i = 0; i <= 62; i++) begin
      if (i == 15) continue;  // readme.md: post_randomize
      after_count = 0;
      armed = 0;
      if (n == null) n = new;
      sorted = '{3, 1, 2};
      q = '{};
      dyn = '{};
      rec.c = '{};
      mem = '{default: 0};
      x = 0; s = "";
      ctx = i;
      victim_p = null;
      fork victim; join_none
      #20;
      $display("%0d %s after=%0d armed=%0d x=%0d q=%p sorted=%p dyn=%0d s=%s aa=%0d mem=%p rec=%p box=%0d sem=%0d wide=%h", i,
        victim_p.status().name(), after_count, armed, x, q, sorted, dyn.size(), s, aa.num(), mem, rec, box.num(), sem.try_get(2), wide);
    end
    $finish;
  end
  task victim;
    victim_p = process::self();
    armed = (ctx != 13);
    body(ctx);
  endtask
  final $display("final %0t", $time);
endmodule
