// IEEE 1800-2009 11.4.13: inside compares nonintegral operands with `==`;
// a chandle selector and item are compared that way (decision R14-D1). The
// file keeps its historical name, which the SYN-038 ledger cites.
module tb;
    chandle handle;
    logic result;

    initial begin
        result = handle inside {handle};
    end
endmodule
