// llg-test-fixture: tests/fixtures/sim/syn038_pairwise/continuous_pattern_lhs.sv
// Positional continuous assignment patterns fan one RHS value out to packed
// net and variable leaves, including selected destinations.

module tb;
  logic [1:0] source = 2'b10;
  wire high;
  wire low;
  wire [1:0] selected_net;
  logic [1:0] pure_variable;
  logic [1:0] selected_variable;

  function automatic logic [1:0] counted_source;
    begin
      $display("rhs_eval");
      counted_source = source;
    end
  endfunction

  assign high = 1'b0;
  assign '{high, low} = source;
  assign '{selected_net[1], selected_net[0]} = source;
  assign '{pure_variable[1], pure_variable[0]} = source;
  assign '{selected_variable[1], selected_variable[0]} = counted_source();

  initial begin
    #1;
    if ({high, low} !== 2'bx0 || selected_net !== 2'b10 ||
        pure_variable !== 2'b10 || selected_variable !== 2'b10)
      $fatal(1, "initial continuous pattern mismatch");
    source = 2'b01;
    #1;
    if ({high, low} !== 2'b01 || selected_net !== 2'b01 ||
        pure_variable !== 2'b01 || selected_variable !== 2'b01)
      $fatal(1, "updated continuous pattern mismatch");
    $display("net=%b%b selected_net=%b pure_variable=%b counted_variable=%b",
             high, low, selected_net, pure_variable, selected_variable);
    $finish(0);
  end
endmodule
