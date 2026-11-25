// Cancellation (9.6, 9.7, 20.2) reached in nested calls, randomize hooks, a
// coverage sample and after a clocking event.
`timescale 1ns/1ns
module tb;
  string act, ctx;
  process main_p;
  bit clk;
  int trace;
  event done_ev;
  always #5 clk = ~clk;

  // The action; a function, so it can be reached from every context.
  function automatic void take();
    case (act)
      "disable": disable victim;
      "kill": begin process p; p = process::self(); p.kill(); end
      "fork": begin process p; p = process::self(); end
      "finish": $finish;
      "fatal": $fatal(1, "fatal reached");
      "stop": $stop;
      "exit": ;
    endcase
  endfunction

  function automatic int nested(int depth);
    int below;
    if (depth > 0) begin
      below = nested(depth - 1);
      return below + 1;
    end
    $display("[%0t] nested: take", $time);
    take();
    $display("[%0t] nested: after take", $time);
    return 0;
  endfunction

  class item;
    rand int unsigned a;
    string where_;
    constraint c { a inside {[3:3]}; }
    function void pre_randomize();
      if (where_ == "pre") begin
        $display("[%0t] pre: take", $time);
        take();
        $display("[%0t] pre: after take", $time);
      end
    endfunction
    function void post_randomize();
      if (where_ == "post") begin
        $display("[%0t] post: take a=%0d", $time, a);
        take();
        $display("[%0t] post: after take", $time);
      end
    endfunction
  endclass

  int cov_v;
  function automatic int cov_hook(int v);
    $display("[%0t] sample: take", $time);
    take();
    $display("[%0t] sample: after take", $time);
    return v;
  endfunction
  covergroup cg;
    cp: coverpoint cov_hook(cov_v) { bins b[] = {[0:3]}; }
  endgroup
  cg cover_i;

  logic q;
  initial q = 0;

  clocking cb @(posedge clk);
    input q;
  endclocking

  int r;
  item it;
  initial begin
    void'($value$plusargs("act=%s", act));
    void'($value$plusargs("ctx=%s", ctx));
    cover_i = new;
    $display("start act=%s ctx=%s", act, ctx);
    begin : victim
      #1;
      case (ctx)
        "nested": begin r = nested(3); $display("r=%0d", r); end
        "pre", "post": begin
          it = new; it.where_ = ctx;
          r = it.randomize();
          $display("randomize=%0d a=%0d", r, it.a);
        end
        "sample": begin cov_v = 2; cover_i.sample(); $display("sampled"); end
        "clocking": begin @(cb); take(); $display("[%0t] after clocking take", $time); end
      endcase
      $display("[%0t] end of victim", $time);
    end
    $display("[%0t] after victim", $time);
    #20 $display("[%0t] main ends", $time);
    $finish;
  end

  initial begin
    #40 $display("[%0t] bystander", $time);
    #10 $finish;
  end

  final $display("[%0t] final", $time);
endmodule
