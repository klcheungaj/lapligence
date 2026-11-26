// An assignment whose control names a null event ends the run where it is
// issued, whatever process waits for the control; a control that is waited
// for no times is not waited for. FORM selects the assignment.
`ifndef FORM
`define FORM 0
`endif
`timescale 1ns / 1ns

module tb;
  class c;
  endclass
  event empty = null, live;
  c h, none, g = new;
  string sa[2];
  int x;

  function automatic int f(int v);
    return v;
  endfunction

  initial begin
    none <= repeat (x) @empty g;
    #1 $display("before");
    case (`FORM)
      0: h <= @empty g;
      1: sa[x] <= repeat (2) @(posedge x[0] or empty) "s";
      2: ->> @(f(x) or empty) live;
      default: @(f(x) or empty);
    endcase
    $display("went on");
  end

  initial #1 $display("not reached");

  final $display("final %0t %b", $time, none == g);
endmodule
