// Actions taken inside DPI imports and exports (35.9).
`timescale 1ns/1ns
module tb;
  import "DPI-C" context task c_task(input string act, input int depth, output int o);
  import "DPI-C" context function int c_func(input string act, input int depth);
  export "DPI-C" task sv_task;
  export "DPI-C" function sv_func;

  string act, ctx;
  process victim_p;
  int seen, out;
  bit clk;
  always #5 clk = ~clk;

  function automatic void take(string a);
    case (a)
      "disable": disable tb.victim;
      "kill": begin process p; p = process::self(); p.kill(); end
      "killv": victim_p.kill();
      "finish": $finish;
      "fatal": $fatal(1, "fatal reached");
      "stop": $stop;
      default: ;
    endcase
    seen++;
  endfunction

  task automatic sv_task(input string a, input int depth, output int o);
    o = 7;
    if (depth > 0) c_task(a, depth - 1, o);
    else begin
      $display("[%0t] sv_task take %s", $time, a);
      if (a == "wait") begin #3; o = 8; end
      else if (a == "waitkill") begin #3; victim_p.kill(); end
      else if (a == "dfork") disable fork;
      else take(a);
      $display("[%0t] sv_task after take", $time);
    end
    o = o + 1;
  endtask

  function automatic int sv_func(input string a, input int depth);
    if (depth > 0) return c_func(a, depth - 1) + 1;
    $display("[%0t] sv_func take %s", $time, a);
    take(a);
    $display("[%0t] sv_func after take", $time);
    return 5;
  endfunction

  class item;
    rand int unsigned a;
    string where_;
    constraint c { a inside {[3:3]}; }
    function void pre_randomize();
      int r;
      if (where_ == "pre") begin r = c_func(act, 0); $display("pre r=%0d", r); end
    endfunction
    function void post_randomize();
      int r;
      if (where_ == "post") begin r = c_func(act, 0); $display("post r=%0d", r); end
    endfunction
    task run(input int depth);
      int o;
      c_task(act, depth, o);
      $display("method o=%0d", o);
    endtask
  endclass

  covergroup cg with function sample(int x);
    cp: coverpoint x { bins b[] = {[0:7]}; }
  endgroup
  cg cover_i = new;
  item it = new;

  task body;
    int o;
    case (ctx)
      "task": begin c_task(act, 0, o); $display("task o=%0d", o); end
      "nested": begin c_task(act, 2, o); $display("task o=%0d", o); end
      "fork": begin
        fork
          begin c_task(act, 0, o); $display("branch o=%0d", o); end
          begin #2 $display("[%0t] sibling", $time); end
        join
      end
      "func": begin o = c_func(act, 0); $display("func o=%0d", o); end
      "funcn": begin o = c_func(act, 2); $display("func o=%0d", o); end
      "method": it.run(1);
      "pre", "post": begin it.where_ = ctx; o = it.randomize(); $display("rand=%0d a=%0d", o, it.a); end
      "sample": begin cover_i.sample(c_func(act, 0)); $display("sampled"); end
      "expr": begin out = 1 + c_func(act, 0) + c_func("none", 0); $display("out=%0d", out); end
      "nba": begin @(posedge clk); out <= c_func(act, 0); @(posedge clk); $display("out=%0d", out); end
      "final": ;
      default: ;
    endcase
  endtask

  task victim;
    victim_p = process::self();
    #1;
    body;
    $display("[%0t] victim ends", $time);
  endtask

  initial begin
    void'($value$plusargs("act=%s", act));
    void'($value$plusargs("ctx=%s", ctx));
    fork victim; join_none
    #30 $display("[%0t] status=%s seen=%0d out=%0d", $time, victim_p.status().name(), seen, out);
    $finish;
  end
  final begin
    if (ctx == "final") begin int r; r = c_func(act, 1); $display("final r=%0d", r); end
    $display("[%0t] final seen=%0d", $time, seen);
  end
  final $display("last final");
endmodule
