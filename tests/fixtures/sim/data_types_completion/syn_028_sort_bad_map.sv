// IEEE 1800-2009 7.12.2: one fixed-array sort fault, a real-valued map key.
module tb;
    logic [7:0] values [0:1];

    initial begin
        values.sort() with (real'(item));
    end
endmodule
