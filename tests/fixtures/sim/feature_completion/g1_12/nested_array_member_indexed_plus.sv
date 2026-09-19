// llg-test-fixture: tests/fixtures/sim/feature_completion/g1_12/nested_array_member_indexed_plus.sv
// IEEE 1800-2009 11.5.1: `[base +: width]` beneath a fixed-array element
// selects `base+width-1` down to `base`, so both the read and the write keep
// the ascending bit order (a descending `-:` select is covered by the
// sibling fixture).
module tb;
    logic [3:0][7:0] packed_elem [0:1];
    int i;

    initial begin
        packed_elem[1] = 32'h11_22_33_44;
        i = 1;

        // Lane 2 is 0x22 = 0010_0010; bits [4:2] are 001.
        $display("read_plus %h", packed_elem[i][2][2 +: 3]);

        // Lane 0 is 0x44 = 0100_0100; bits [4:2] become 101 -> 0x54.
        packed_elem[i][0][2 +: 3] = 3'b101;
        $display("write_plus %h", packed_elem[1]);

        // Lane 3 is 0x11 = 0001_0001; bits [5:2] become 1001 -> 0x25.
        packed_elem[i][3][2 +: 4] = 4'b1001;
        $display("write_plus2 %h", packed_elem[1]);
        $finish(0);
    end
endmodule
