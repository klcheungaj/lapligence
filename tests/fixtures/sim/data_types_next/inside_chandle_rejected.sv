// IEEE 1800-2009 11.4.13: inside operands must be numeric or string values;
// an opaque chandle is one independent illegal context.
module tb;
    chandle handle;
    logic result;

    initial begin
        result = handle inside {handle};
    end
endmodule
