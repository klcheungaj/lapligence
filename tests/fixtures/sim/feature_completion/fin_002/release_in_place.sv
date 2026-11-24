// FIN-002 regression of the release rule (decision CS-D3) for the forms the
// portable decision case leaves out: a release made in a function, in a
// class method and in a final procedure, an element of an array kept in a
// keyed container (more than 4096 cells), a real variable, and a continuous
// assignment whose own function releases its target while it is evaluated.
// Each released variable holds the driven value in the statement after the
// release. Every expected line is derived in readme.md.
`timescale 1ns / 1ns

module tb;
  logic [7:0] in;
  logic [7:0] by_function, by_method, by_final, by_driver;
  logic [7:0] big[5000];
  logic release_inside;
  real ratio;

  assign by_function = in + 8'd1;
  assign by_method = in + 8'd2;
  assign by_final = in + 8'd3;
  assign big[4000] = in + 8'd4;
  assign big[7] = in + 8'd5;
  assign ratio = in * 1.5;

  // The driver of `by_driver` releases it once from inside its evaluation.
  function automatic logic [7:0] driven(input logic [7:0] value);
    if (release_inside) begin
      release_inside = 1'b0;
      release by_driver;
    end
    return value + 8'd6;
  endfunction
  assign by_driver = driven(in);

  function void release_in_function();
    release by_function;
    $display("in the function: %0d", by_function);
  endfunction

  class releaser;
    function void run();
      release by_method;
      $display("in the method: %0d", by_method);
    endfunction
  endclass
  releaser object = new();

  initial begin
    release_inside = 1'b0;
    in = 8'd1;
    #1 force by_function = 8'd201;
    force by_method = 8'd202;
    force by_final = 8'd203;
    force big[4000] = 8'd204;
    force big[7] = 8'd205;
    force by_driver = 8'd206;
    force ratio = 99.5;
    #1 in = 8'd2;
    #1 release_in_function();
    object.run();
    release big[4000];
    $display("one element: %0d,%0d", big[4000], big[7]);
    release big[7];
    $display("both elements: %0d,%0d", big[4000], big[7]);
    release ratio;
    $display("real: %0.1f", ratio);
    release_inside = 1'b1;
    in = 8'd3;
    #1 $display("released by its driver: %0d", by_driver);
    in = 8'd4;
    #1 $display("next operand: %0d", by_driver);
    $finish;
  end

  final begin
    release by_final;
    $display("in the final procedure: %0d", by_final);
  end
endmodule
