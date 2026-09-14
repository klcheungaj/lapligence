// llg-test-fixture: tests/fixtures/sim/review_batch4/packed_overlap.sv
module tb;
    logic a = 0, b = 0;
    logic [3:0] q;
    always_comb q[2:0] = {a,a,a};
    always_comb q[3:2] = {b,b};
endmodule
