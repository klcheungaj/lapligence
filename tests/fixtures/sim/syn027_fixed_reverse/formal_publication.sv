// IEEE 1800-2009 7.12.2 and 13.5: value, copy-back and reference formals.
`timescale 1ns/1ns
module tb;
  typedef logic [7:0] matrix_t [0:1][0:2];
  matrix_t values;
  logic [7:0] observed;
  logic [7:0] input_result;
  integer selections;

  function automatic integer pick(input integer row);
    selections = selections + 1;
    return row;
  endfunction

  function automatic logic [7:0] reverse_input(input matrix_t copy);
    copy[pick(0)].reverse();
    return copy[0][0];
  endfunction

  task automatic reverse_inout(inout matrix_t copy);
    copy[pick(0)].reverse();
  endtask

  task automatic reverse_ref(ref matrix_t copy);
    copy[pick(1)].reverse();
  endtask

  task automatic forward_ref(ref matrix_t copy);
    reverse_ref(copy);
  endtask

  assign observed = values[1][0];

  initial begin
    values[0] = '{8'd1, 8'd2, 8'd3};
    values[1] = '{8'd4, 8'd5, 8'd6};
    #1;
    if (observed !== 8'd4) $fatal(1, "initial reference consumer");
    selections = 0;
    input_result = reverse_input(values);
    if (selections != 1 || input_result !== 8'd3 ||
        values[0][0] !== 8'd1 || values[0][2] !== 8'd3)
      $fatal(1, "input formal is a private value");
    reverse_inout(values);
    if (selections != 2 || values[0][0] !== 8'd3 ||
        values[0][1] !== 8'd2 || values[0][2] !== 8'd1 ||
        values[1][0] !== 8'd4)
      $fatal(1, "inout formal copy back");
    forward_ref(values);
    #1;
    if (selections != 3 || observed !== 8'd6 ||
        values[1][0] !== 8'd6 || values[1][1] !== 8'd5 ||
        values[1][2] !== 8'd4 || values[0][0] !== 8'd3)
      $fatal(1, "forwarded ref publication or neighbor");
    $display("PASS syn027_reverse_formals");
    $finish;
  end
endmodule
