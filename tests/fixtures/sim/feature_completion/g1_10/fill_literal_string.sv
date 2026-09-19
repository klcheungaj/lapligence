// IEEE 1800-2009 5.7.1: an unbased unsized fill literal stands for a packed
// bit value and has no implicit conversion to a string. This fixture contains
// one fault and must stay rejected.
module tb;
    string text;
    initial begin
        text = '1;
        $finish;
    end
endmodule
