// SIM-099 step 2: run-time reports name the source line of the class
// method, package function, interface assertion or randomize call that
// produced them, also when the model is generated after the frontend
// snapshot and the owned database are destroyed.
`timescale 1ns / 1ns

package checks_pkg;
  function automatic void check_positive(int x);
    assert (x > 0)
    else $error("not positive: %0d", x);
  endfunction
endpackage

interface flag_if (input logic clk);
  logic flag;
  always_low : assert property (@(posedge clk) !flag);
endinterface

class limited_c;
  rand bit [3:0] v;
  constraint below_four {v < 4;}
  function void warn_once(int k);
    $warning("limited_c warning %0d", k);
  endfunction
endclass

module tb;
  import checks_pkg::*;
  logic clk = 1'b0;
  flag_if fi (clk);
  limited_c obj;
  int ok;

  initial begin
    fi.flag = 1'b0;
    obj = new;
    check_positive(3);
    check_positive(-2);
    obj.warn_once(1);
    ok = obj.randomize() with { v > 8; };
    $display("randomize ok=%0d", ok);
    #4 fi.flag = 1'b1;
    #1 clk = 1'b1;
    #1 $display("done");
    $finish;
  end
endmodule
