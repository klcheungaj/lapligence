// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_047/trace_values.sv
// SIM-047 A01: the VCD header, alias identities, values and timestamps of
// ports, nets, variables, a real and a fixed array, with generate scopes
// named by IEEE 1800-2009 27.4 and 27.6. The $strobe lines print the
// end-of-step values the VCD must record.
`timescale 1ns/1ps
module stage (input logic clk, input logic drive, input logic [3:0] d,
              output logic [3:0] q, inout wire bus);
    always @(posedge clk) q <= d;
    assign bus = drive ? q[0] : 1'bz;
endmodule

module tb;
    logic clk;
    logic [3:0] d;
    wire [3:0] q0;
    wire [3:0] q1;
    wire shared;
    real level;
    logic [7:0] mem [1:0];
    logic glitch;

    for (genvar i = 0; i < 2; i++) begin : g
        if (i == 0) begin : first
            stage s(.clk(clk), .drive(1'b1), .d(d), .q(q0), .bus(shared));
        end else begin : second
            stage s(.clk(clk), .drive(1'b0), .d(~d), .q(q1), .bus(shared));
        end
    end
    if (1) begin
        wire extra = clk;
    end

    task show;
        $strobe("%0t clk=%b d=%b q0=%b q1=%b shared=%b glitch=%b mem=%h,%h level=%.17g",
                $time, clk, d, q0, q1, shared, glitch, mem[1], mem[0], level);
    endtask

    initial begin
        clk = 1'b0;
        d = 4'h0;
        level = 0.5;
        glitch = 1'b0;
        mem[0] = 8'h00;
        mem[1] = 8'hff;
        $dumpfile("trace.vcd");
        $dumpvars(0, tb);
        d = 4'h3;
        show;
        #1 clk = 1'b1;
        glitch = 1'b1;
        glitch = 1'b0;
        show;
        #1 clk = 1'b0;
        d = 4'b1x0z;
        level = 1.0 / 3.0;
        mem[1] = 8'h5a;
        show;
        #1 clk = 1'b1;
        show;
        #1 $finish;
    end
endmodule
