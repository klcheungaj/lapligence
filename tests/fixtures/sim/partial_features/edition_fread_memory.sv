// llg-test-fixture: tests/fixtures/sim/partial_features/edition_fread_memory.sv
// IEEE 1364-2001 17.2.4.4 and IEEE 1800-2009 21.3.4.4: a memory
// argument is storage, not an illegal whole-array value in edition 2001.
module tb;
    reg [7:0] descending [3:1];
    reg [7:0] ascending [1:3];
    reg [15:0] packed_value;
    integer fd, count, partial, packed_count;
    initial begin
        fd = $fopen("edition_fread.bin", "wb");
        $fwrite(fd, "ABC");
        $fclose(fd);
        fd = $fopen("edition_fread.bin", "rb");
        count = $fread(descending, fd);
        $fclose(fd);
        ascending[1] = 8'hff;
        ascending[2] = 8'hff;
        ascending[3] = 8'hff;
        fd = $fopen("edition_fread.bin", "rb");
        partial = $fread(ascending, fd, 2, 2);
        $fclose(fd);
        $display("fread=%0d desc=%h/%h/%h partial=%0d asc=%h/%h/%h",
                 count, descending[1], descending[2], descending[3],
                 partial, ascending[1], ascending[2], ascending[3]);
        fd = $fopen("edition_fread.bin", "rb");
        // Packed reads ignore both bounds, including a zero memory count.
        packed_count = $fread(packed_value, fd, 999, 0);
        $display("packed=%h bytes=%0d", packed_value, packed_count);
        $fclose(fd);
        fd = $fopen("edition_fread.bin", "rb");
        packed_count = $fread(packed_value, fd, , 0);
        $display("omitted=%h bytes=%0d", packed_value, packed_count);
        $fclose(fd);
        $finish(0);
    end
endmodule
