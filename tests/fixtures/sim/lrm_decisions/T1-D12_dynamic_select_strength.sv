// IEEE 1800-2009 6.3.2 L4553-4555 (SystemVerilog-1800-2009.txt): "The language
// includes strength information in addition to the basic value information
// for nets." "The additional strength information associated with bits of a
// net is not considered part of the data type."
// 9.4.2 L11767-11769: "The value changes on nets and variables can be used as
// events to trigger the execution of a statement."
// 21.2.1.5 L36265-36266: "The %v format specification is used to display the
// strength of scalar nets."
// The text does not say which strength a select with a variable index has,
// nor whether its strength-only change is an event.
// Decision: a dynamic bit or indexed part-select of a net shows, under %v,
// the strength of the bits its index selects now. An event control on it,
// also one with a function call in the index, and a $monitor argument naming
// it occur on a strength-only change of those bits, including one that only a
// change of the index brings. Bits past the net and an unknown index read StX.
`timescale 1ns/1ns

module tb;
  wire [1:0] v;
  wire [4:1] u;
  wire [1:4] r;
  reg a = 1'b0, b = 1'bz;
  int idx = 0, k = 1, m = 2;
  int calls = 0;

  bufif1 (weak0, weak1) va (v[0], a, 1'b1);
  bufif1 (strong0, strong1) vb (v[0], b, b !== 1'bz);
  assign v[1] = 1'b0;

  bufif1 (weak0, weak1) ua (u[2], a, 1'b1);
  bufif1 (strong0, strong1) ub (u[2], b, b !== 1'bz);
  assign u[1] = 1'b1;
  bufif1 (pull0, pull1) up (u[4], 1'b1, 1'b1);
  assign u[3] = 1'bz;

  bufif1 (weak0, weak1) ra (r[3], a, 1'b1);
  bufif1 (strong0, strong1) rb (r[3], b, b !== 1'bz);
  assign r[1:2] = 2'b10;
  assign r[4] = 1'b1;

  function automatic int pick(input int i);
    calls = calls + 1;
    return i;
  endfunction

  string seen[6];

  always @(v[idx]) seen[0] = $sformatf("v[idx]=%b %v", v[idx], v[idx]);
  always @(u[k +: 2]) seen[1] = $sformatf("u[k+:2]=%b %v %v", u[k +: 2], u[k + 1], u[k]);
  always @(u[m -: 2]) seen[2] = $sformatf("u[m-:2]=%b %v %v", u[m -: 2], u[m], u[m - 1]);
  always @(r[k]) seen[3] = $sformatf("r[k]=%b %v", r[k], r[k]);
  always @(r[m +: 2]) seen[4] = $sformatf("r[m+:2]=%b %v %v", r[m +: 2], r[m], r[m + 1]);
  always @(v[pick(idx)]) seen[5] = $sformatf("v[pick]=%b", v[idx]);

  initial $monitor("%0t monitor v[idx]=%b", $time, v[idx]);

  initial begin
    #2 b = 1'b0;
    #2 b = 1'bz;
    #2 a = 1'bx;
    #2 b = 1'b1;
    #2 b = 1'bz; a = 1'b0;
    #2 idx = 1; k = 3; m = 3;
    #2 b = 1'b0;
    #2 idx = 0; k = 4; m = 4;
    #2 b = 1'bz;
    #2 idx = 5; k = 0; m = 5;
    #2 idx = -1; k = -1; m = 1;
    #2 $display("%0t end", $time);
    $finish;
  end

  initial begin
    #1;
    forever begin
      $write("%0t:", $time);
      for (int i = 0; i < 6; i++) begin
        if (seen[i] != "") $write(" {%s}", seen[i]);
        seen[i] = "";
      end
      $display("");
      #2;
    end
  end
endmodule
