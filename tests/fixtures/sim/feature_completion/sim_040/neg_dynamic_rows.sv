// SIM-040 negative: a dynamic array of unpacked arrays is not marshalled as the
// actual of an open-array formal.
module tb;
    import "DPI-C" function void neg_rows(input int a [][]);
    int rows [][3];
    initial begin
        rows = new[2];
        neg_rows(rows);
    end
endmodule
