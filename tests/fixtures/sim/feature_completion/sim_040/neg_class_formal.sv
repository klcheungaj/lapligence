// SIM-040 negative: a class handle is not a legal DPI-C formal type (35.5.6).
module tb;
    class item;
    endclass
    import "DPI-C" function void neg_class(input item e);
    item x;
    initial neg_class(x);
endmodule
