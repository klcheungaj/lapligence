// Actions taken in program processes and in processes that a nonblocking
// assignment or a delayed trigger wakes.
`timescale 1ns/1ns
module tb;
  string act, ctx;
  bit clk;
  logic [7:0] q, d, r;
  int woke, late;
  process p_nba;
  event ev;
  always #5 clk = ~clk;
  initial begin q = 0; d = 1; end
  always @(posedge clk) q <= q + 1;
  function automatic void take(string where_);
    $display("[%0t] take %s %s", $time, act, where_);
    case (act)
      "kill": begin process p; p = process::self(); p.kill(); end
      "killn": p_nba.kill();
      "disable": disable tb.watcher;
      "disable_w": disable tb.writer;
      "finish": $finish;
      "fatal": $fatal(1, "stop here");
      "stop": $stop;
      default: ;
    endcase
    $display("[%0t] after take %s", $time, where_);
  endfunction
  task watcher;
    p_nba = process::self();
    forever begin
      @(q);
      woke++;
      if (ctx == "nba" && q == 3) take("nba");
    end
  endtask
  task writer;
    #21;
    r <= #10 8'h55;
    d <= 8'h77;
    ->> #4 ev;
    if (ctx == "pending") take("pending");
    #1 $display("[%0t] writer went on", $time);
  endtask
  always @(ev) begin late++; if (ctx == "ev") take("ev"); end
  always @(r) if ($time > 0) begin late += 10; if (ctx == "latenba") take("latenba"); end
  initial begin
    void'($value$plusargs("act=%s", act));
    void'($value$plusargs("ctx=%s", ctx));
    fork watcher; writer; join_none
    #5;
    $strobe("[%0t] strobe q=%0d", $time, q);
    #60 $display("[%0t] end woke=%0d late=%0d r=%h d=%h q=%0d w=%s", $time, woke, late, r, d, q, p_nba.status().name());
    $finish;
  end
  always @(posedge clk) if (q == 2) $strobe("[%0t] strobe2 q=%0d", $time, q);
  pgm p1();
  final $display("[%0t] final woke=%0d late=%0d", $time, woke, late);
endmodule
program automatic pgm;
  initial begin
    fork
      begin
        #13;
        if (tb.ctx == "prog") begin tb.take("prog"); $display("prog went on"); end
        if (tb.ctx == "exit") begin $display("exit"); $exit; $display("exit went on"); end
        if (tb.ctx == "progfork") begin
          fork #3 $display("prog child"); join_none
          #1 disable fork;
          $display("prog disabled fork");
        end
      end
      begin @(tb.q); tb.woke += 100; end
    join
    $display("[%0t] program ends", $time);
    #100;
  end
  final $display("program final");
endprogram
