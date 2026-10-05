// Value-backend witness: 65-bit multiplication, the first width above the
// inline 64-bit representation. Each round computes one product into a named
// destination and one product consumed as a fresh temporary owner.

`ifndef LLG_VB_ROUNDS
`define LLG_VB_ROUNDS 2000000
`endif

module mul65 #(
    parameter int ROUNDS = `LLG_VB_ROUNDS
);
    logic [64:0] a;
    logic [64:0] b;
    logic [64:0] p;
    logic [64:0] q;

    initial begin
        a = 65'h1_2345_6789_abcd_ef01;
        b = 65'h0_fedc_ba98_7654_3211;
        q = '0;
        for (int i = 0; i < ROUNDS; i++) begin
            p = a * b;
            q = q ^ ((a * p) + b);
            a = p + 65'd7;
            b = b ^ {p[31:0], q[32:0]};
        end
        $display("mul65 rounds=%0d p=%h q=%h", ROUNDS, p, q);
        $finish(0);
    end
endmodule
