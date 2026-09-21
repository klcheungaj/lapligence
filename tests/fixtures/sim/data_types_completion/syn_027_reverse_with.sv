// IEEE 1800-2009 7.12.2: reverse with-clause is one unsupported fixed-array
// method form.
module tb;
    logic [7:0] values [0:1];

    initial begin
        values.reverse() with (item);
    end
endmodule
