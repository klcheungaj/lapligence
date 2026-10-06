// SIM-011 boundary: a static class property of a record type with a string
// member is legal (SV 8.9) but is rejected with a dedicated diagnostic.
typedef struct {
    string s;
    int n;
} ns_t;

class holder_c;
    static ns_t shared;
endclass

module tb;
    initial begin
        holder_c::shared.s = "x";
        $display("%s", holder_c::shared.s);
    end
endmodule
