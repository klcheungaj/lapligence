// llg-test-fixture: tests/fixtures/sim/syn038_pairwise/function_inputs_from_drivers.sv
module tb;
    logic clk = 1'b0;
    logic [7:0] nba_source = 8'h11;
    logic [7:0] seed = 8'h11;
    wire [7:0] net_source;
    logic [7:0] variable_source;
    logic [7:0] nba_observed;
    logic [7:0] net_observed;
    logic [7:0] variable_observed;
    logic [7:0] control = 8'hc3;

    // Focal vector: integral_bit_logic, direct_projection, call_argument,
    // whole_object, module_package, input, function, module, local, none,
    // none, continuous_net, initial.
    assign net_source = seed;

    // Focal vector: integral_bit_logic, direct_projection, call_argument,
    // whole_object, module_package, input, function, module, local, none,
    // none, continuous_variable, initial.
    assign variable_source = seed;

    // Focal vector: integral_bit_logic, direct_projection, call_argument,
    // whole_object, module_package, input, function, module, local, none,
    // none, procedural_nba, initial.
    always_ff @(posedge clk)
        nba_source <= 8'h5a;

    function automatic logic [7:0] echo(input logic [7:0] value);
        return value;
    endfunction

    initial begin
        nba_observed = echo(nba_source);
        net_observed = echo(net_source);
        variable_observed = echo(variable_source);
        if (nba_observed !== 8'h11 || net_observed !== 8'h11 ||
            variable_observed !== 8'h11 || control !== 8'hc3)
            $fatal(1, "function-input baseline mismatch");

        #1;
        clk = 1'b1;
        seed = 8'h5a;
        #1;
        nba_observed = echo(nba_source);
        net_observed = echo(net_source);
        variable_observed = echo(variable_source);
        if (nba_observed !== 8'h5a || net_observed !== 8'h5a ||
            variable_observed !== 8'h5a || control !== 8'hc3)
            $fatal(1, "function-input update mismatch");

        $display("function_inputs=%02h/%02h/%02h", nba_observed, net_observed,
            variable_observed);
        $finish(0);
    end
endmodule
