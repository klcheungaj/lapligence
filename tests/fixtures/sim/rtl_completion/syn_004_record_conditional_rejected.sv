// SYN-004 boundary: native record members remain outside the fixed payload
// path. LRM: IEEE 1800-2009 7.2 and 11.4.11.
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
        $finish(0);
    end
endmodule
