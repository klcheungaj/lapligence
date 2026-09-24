// llg-test-fixture: tests/fixtures/sim/syn038_pairwise/child_param_decl_init.sv
// A parent variable initializer reads elaborated parameters from a child;
// sibling instances keep their own overridden parameter values.
module fixed_child;
    localparam logic [7:0] LIMIT = 8'h05;
endmodule

module parameter_child #(parameter logic [7:0] SEED = 8'h00);
    localparam logic [7:0] LIMIT = SEED;
endmodule

module tb;
    fixed_child u();
    parameter_child #(.SEED(8'h05)) u0();
    parameter_child #(.SEED(8'h0a)) u1();

    logic [7:0] declaration_read = u.LIMIT;
    logic [7:0] first_sibling = u0.LIMIT;
    logic [7:0] second_sibling = u1.LIMIT;
    logic [7:0] procedural_control;

    initial begin
        procedural_control = u.LIMIT;
        if (declaration_read !== 8'h05 || first_sibling !== 8'h05 ||
            second_sibling !== 8'h0a || procedural_control !== 8'h05)
            $fatal(1, "child parameter initializer mismatch");
        $display("decl=%h siblings=%h/%h procedural=%h", declaration_read,
                 first_sibling, second_sibling, procedural_control);
        $finish(0);
    end
endmodule
