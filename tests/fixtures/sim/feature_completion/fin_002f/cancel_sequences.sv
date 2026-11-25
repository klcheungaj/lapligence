// Actions taken in assertion actions, expect and a sequence event control.
`timescale 1ns/1ns
module tb;
  string act, ctx;
  bit clk;
  bit a, b, c;
  int hits;
  process p_main, p_wait;
  always #5 clk = ~clk;
  always @(posedge clk) begin
    a <= ($time > 10);
    b <= ($time > 20);
    c <= ($time > 30);
  end

  function automatic int take(string here);
    if (ctx == here) begin
      hits++;
      $display("[%0t] take %s %s", $time, act, here);
      case (act)
        "kill": p_wait.kill();
        "disable": disable tb.waiter;
        "finish": $finish;
        "fatal": $fatal(1, "stop");
        "stop": $stop;
        default: ;
      endcase
    end
    return 1;
  endfunction

  sequence s_ab;
    int v;
    @(posedge clk) (a, v = take("match")) ##1 (b, v = take("match2"));
  endsequence

  property p_ab;
    @(posedge clk) $rose(a) |-> ##1 b ##1 c;
  endproperty

  ap: assert property (p_ab) begin void'(take("pass")); end else begin void'(take("fail")); end
  cp: cover property (s_ab) begin void'(take("cover")); end

  task waiter;
    p_wait = process::self();
    @(s_ab) $display("[%0t] waiter saw s_ab", $time);
    expect (@(posedge clk) ##1 c) begin
      void'(take("expect"));
      $display("[%0t] expect passed", $time);
    end else $display("[%0t] expect failed", $time);
    $display("[%0t] waiter done", $time);
  endtask

  initial begin
    void'($value$plusargs("act=%s", act));
    void'($value$plusargs("ctx=%s", ctx));
    fork waiter; join_none
    #27;
    if (ctx == "outside") begin
      if (act == "kill") p_wait.kill();
      if (act == "disable") disable waiter;
    end
    if (ctx == "outside2") begin
      #20;
      if (act == "kill") p_wait.kill();
      if (act == "disable") disable waiter;
    end
    #73;
    $display("[%0t] end hits=%0d wait=%s", $time, hits, p_wait.status().name());
    $finish;
  end
  final $display("[%0t] final", $time);
endmodule
