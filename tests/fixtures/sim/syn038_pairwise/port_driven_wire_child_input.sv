// llg-test-fixture: tests/fixtures/sim/syn038_pairwise/port_driven_wire_child_input.sv
module leaf(input wire [7:0] value, output wire [7:0] echoed);
    assign echoed = value;
endmodule

module tb;
    logic [7:0] seed;
    wire [7:0] source;
    logic [7:0] control = 8'hc3;
    leaf child(.value(source), .echoed());

    // Focal vector: integral_bit_logic, direct_projection, port_actual,
    // whole_object, module_package, input, module, child_port, none, none,
    // none, continuous_net, none.
    assign source = seed;

    initial begin
        seed = 8'h00;
        #1;
        if (source !== 8'h00 || child.value !== 8'h00 || control !== 8'hc3)
            $fatal(1, "wire child-input baseline mismatch");

        seed = 8'h5a;
        #1;
        if (source !== 8'h5a || child.value !== 8'h5a || control !== 8'hc3)
            $fatal(1, "wire child-input update mismatch");

        $display("port_source=%h/%h", source, child.value);
        $finish(0);
    end
endmodule
