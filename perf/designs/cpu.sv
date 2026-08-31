// Copied verbatim from tests/sim_stress.rs `stress_cpu_lite_datapath`.
module alu (
    input  logic [1:0] op,
    input  logic [3:0] a,
    input  logic [3:0] b,
    output logic [3:0] y,
    output logic eq
);
    always_comb begin
        eq = (a == b);
        case (op)
            2'd0: y = a + b;
            2'd1: y = a - b;
            2'd2: y = a & b;
            default: y = a | b;
        endcase
    end
endmodule

module cpu_lite (
    input  logic clk, rst_n, start,
    output logic [3:0] ra, rb,
    output logic [3:0] alu_y,
    output logic eq,
    output logic done
);
    reg [3:0] state;
    reg [3:0] regfile [0:1];
    reg [3:0] alu_a, alu_b;
    reg [1:0] alu_op;
    wire [3:0] y;
    wire eq_w;

    alu u_alu(.op(alu_op), .a(alu_a), .b(alu_b), .y(y), .eq(eq_w));
    assign alu_y = y;
    assign eq = eq_w;

    always @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            state <= 4'd0;
            regfile[0] <= 4'd0;
            regfile[1] <= 4'd0;
            ra <= 4'd0;
            rb <= 4'd0;
            done <= 1'b0;
        end else begin
            ra <= regfile[0];
            rb <= regfile[1];
            case (state)
                4'd0: if (start) state <= 4'd1;
                4'd1: begin regfile[0] <= 4'd3; state <= 4'd2; end
                4'd2: begin regfile[1] <= 4'd5; state <= 4'd3; end
                4'd3: begin alu_a <= regfile[0]; alu_b <= regfile[1]; alu_op <= 2'd0; state <= 4'd4; end
                4'd4: begin regfile[0] <= y; state <= 4'd5; end
                4'd5: begin alu_a <= regfile[0]; alu_b <= regfile[1]; alu_op <= 2'd1; state <= 4'd6; end
                4'd6: begin regfile[1] <= y; state <= 4'd7; end
                4'd7: begin alu_a <= regfile[0]; alu_b <= regfile[1]; alu_op <= 2'd2; state <= 4'd8; end
                4'd8: begin regfile[0] <= y; state <= 4'd9; end
                4'd9: begin alu_a <= regfile[0]; alu_b <= regfile[1]; alu_op <= 2'd3; state <= 4'd10; end
                4'd10: begin regfile[1] <= y; state <= 4'd11; end
                4'd11: begin alu_a <= regfile[0]; alu_b <= regfile[0]; alu_op <= 2'd3; state <= 4'd12; end
                4'd12: begin done <= 1'b1; state <= 4'd0; end
                default: state <= 4'd0;
            endcase
        end
    end
endmodule

module tb;
    reg clk, rst_n, start;
    wire [3:0] ra, rb, alu_y;
    wire eq, done;

    cpu_lite u_cpu(.clk(clk), .rst_n(rst_n), .start(start),
                   .ra(ra), .rb(rb), .alu_y(alu_y), .eq(eq), .done(done));

    always #5 clk = ~clk;

    initial begin
        clk = 0; rst_n = 1; start = 0;
        #1 rst_n = 0;
        #3 rst_n = 1;
        #6 start = 1;
        #10 start = 0;
        #7 $display("t=27 ra=%0d rb=%0d alu_y=%0d eq=%b done=%b", ra, rb, alu_y, eq, done);
        #10 $display("t=37 ra=%0d rb=%0d alu_y=%0d eq=%b done=%b", ra, rb, alu_y, eq, done);
        #10 $display("t=47 ra=%0d rb=%0d alu_y=%0d eq=%b done=%b", ra, rb, alu_y, eq, done);
        #10 $display("t=57 ra=%0d rb=%0d alu_y=%0d eq=%b done=%b", ra, rb, alu_y, eq, done);
        #10 $display("t=67 ra=%0d rb=%0d alu_y=%0d eq=%b done=%b", ra, rb, alu_y, eq, done);
        #10 $display("t=77 ra=%0d rb=%0d alu_y=%0d eq=%b done=%b", ra, rb, alu_y, eq, done);
        #10 $display("t=87 ra=%0d rb=%0d alu_y=%0d eq=%b done=%b", ra, rb, alu_y, eq, done);
        #10 $display("t=97 ra=%0d rb=%0d alu_y=%0d eq=%b done=%b", ra, rb, alu_y, eq, done);
        #10 $display("t=107 ra=%0d rb=%0d alu_y=%0d eq=%b done=%b", ra, rb, alu_y, eq, done);
        #10 $display("t=117 ra=%0d rb=%0d alu_y=%0d eq=%b done=%b", ra, rb, alu_y, eq, done);
        #10 $display("t=127 ra=%0d rb=%0d alu_y=%0d eq=%b done=%b", ra, rb, alu_y, eq, done);
        #10 $display("t=137 ra=%0d rb=%0d alu_y=%0d eq=%b done=%b", ra, rb, alu_y, eq, done);
        #10 $display("t=147 ra=%0d rb=%0d alu_y=%0d eq=%b done=%b", ra, rb, alu_y, eq, done);
        $finish;
    end
endmodule