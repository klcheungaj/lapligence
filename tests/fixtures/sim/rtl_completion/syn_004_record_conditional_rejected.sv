// SYN-004 boundary: native record members remain outside the fixed payload
// path; SIM-004 merges them member by member on the native record path.
// LRM: IEEE 1800-2009 7.2 and 11.4.11. With an unknown selector the equal
// real members (0.0) survive and the X data member takes its default X.
module tb;
    typedef struct {
        real value;
        logic [7:0] data;
    } record_t;
    record_t left, right, result;
    logic selector;
    logic [7:0] data;

    function automatic record_t choose(input logic sel, input record_t a, b);
        return sel ? a : b;
    endfunction

    function automatic logic [7:0] consume(input record_t value);
        return value.data;
    endfunction

    initial begin
        data = consume(choose(selector, left, right));
        $display("%h", data);
        $finish(0);
    end
endmodule
