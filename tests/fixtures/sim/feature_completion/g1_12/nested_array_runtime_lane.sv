// llg-test-fixture: tests/fixtures/sim/feature_completion/g1_12/nested_array_runtime_lane.sv
// IEEE 1800-2009 7.4, 10.10 and 11.5: a runtime index selecting a whole
// packed dimension of a fixed-array element reads and writes exactly that
// lane, located by declared order (ascending ranges count from the LSB).
module tb;
    logic [3:0][7:0] packed_elem [0:1];
    logic [0:3][7:0] asc_elem [0:1];
    logic [7:0] bits [0:1];
    integer i;
    integer j;

    initial begin
        packed_elem[0] = 32'h1122_3344;
        packed_elem[1] = 32'haabb_ccdd;
        asc_elem[1] = 32'h1122_3344;
        bits[1] = 8'h96;
        i = 1;

        // Descending element range [3:0]: lane 0 is the LSB byte.
        j = 0;
        $display("desc %h", packed_elem[i][j]);
        j = 1;
        $display("desc %h", packed_elem[i][j]);
        j = 2;
        $display("desc %h", packed_elem[i][j]);
        j = 3;
        $display("desc %h", packed_elem[i][j]);

        // Ascending element range [0:3]: lane 0 is the most significant byte.
        j = 0;
        $display("asc %h", asc_elem[i][j]);
        j = 1;
        $display("asc %h", asc_elem[i][j]);
        j = 2;
        $display("asc %h", asc_elem[i][j]);
        j = 3;
        $display("asc %h", asc_elem[i][j]);

        // A single packed dimension still selects one bit.
        j = 0;
        $display("bit %h", bits[i][j]);
        j = 1;
        $display("bit %h", bits[i][j]);
        j = 7;
        $display("bit %h", bits[i][j]);

        // Writes touch only the selected lane.
        j = 2;
        packed_elem[i][j] = 8'h5a;
        $display("wrdesc %h", packed_elem[1]);
        j = 1;
        asc_elem[i][j] = 8'h7e;
        $display("wrasc %h", asc_elem[1]);
        j = 7;
        bits[i][j] = 1'b0;
        $display("wrbit %h", bits[1]);

        // The unselected element is untouched.
        $display("other %h", packed_elem[0]);
        $finish(0);
    end
endmodule
