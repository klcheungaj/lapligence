// llg-test-fixture: tests/fixtures/sim/feature_completion/g1_12/selection_unknown_bounds.sv
// IEEE 1800-2009 7.4.6 and 11.5.3: an out-of-range or unknown array index
// makes a selected assignment a no-op; it must not touch host memory or a
// neighbouring element.
module tb;
    logic [3:0][7:0] packed_elem [0:1];
    logic [7:0] mem [0:1];
    integer i;

    initial begin
        packed_elem[0] = 32'h1122_3344;
        packed_elem[1] = 32'haabb_ccdd;
        mem[0] = 8'h5a;
        mem[1] = 8'h3c;

        i = 5;
        packed_elem[i][1][7:4] = 4'hf;
        mem[i][3:0] = 4'hf;
        $display("bounds %h %h %h", packed_elem[0], packed_elem[1], mem[1]);

        i = 32'bxxxxxxxx;
        packed_elem[i][1] = 8'h00;
        mem[i][3:0] = 4'b0000;
        $display("unknown %h %h %h", packed_elem[0], packed_elem[1], mem[1]);

        // An in-range selected read still observes the element value.
        i = 1;
        $display("read %h %h", mem[1][3:0], packed_elem[1][3:0]);
        $finish(0);
    end
endmodule
