// IEEE 1800-2009 7.5: a dynamic-array new[] size must be a known,
// nonnegative integral value. This fixture contains one runtime size fault.
module tb;
    int values[];
    int bad_size;

    initial begin
        bad_size = -1;
        values = new[bad_size];
    end
endmodule
