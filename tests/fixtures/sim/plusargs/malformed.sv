// IEEE 1800-2009 21.6: a value plusarg format contains one conversion.
module tb;
    logic [7:0] value;
    initial value = $value$plusargs("bad", value);
endmodule
