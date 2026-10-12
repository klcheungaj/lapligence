// SIM-040 negative: unpacked aggregates with string elements are rejected (H.7.8 needs
// a C layout llg does not marshal).
module tb;
    import "DPI-C" function void neg_strings(input string a [2]);
    string a [2];
    initial neg_strings(a);
endmodule
