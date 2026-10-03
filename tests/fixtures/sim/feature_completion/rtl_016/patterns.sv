// IEEE 1800-2009 12.6, 7.3.2, 7.2.1: tagged patterns over unpacked fixed
// payloads check the tag before payload members, compare each member in its
// own state domain and bind payloads with the member type. A binding is
// visible to later &&& clauses and the true arm only; casez/casex wildcard
// tag bits as the enclosing case mode requires.
typedef struct { bit [3:0] flags; logic [7:0] data; } frame_t;

typedef union tagged {
    void Empty;
    frame_t Frame;
    logic [7:0] Pair [0:1];
    union tagged { logic [3:0] Low; logic [3:0] High; } Nested;
} message_t;

module tb;
    message_t message;
    int calls, hits;
    logic [7:0] result;

    function automatic message_t sample();
        calls++;
        return message;
    endfunction

    initial begin
        calls = 0;
        hits = 0;
        message = tagged Frame '{flags: 4'b1x0z, data: 8'h5a};
        // The two-state flags member holds 4'b1000.
        if (sample() matches tagged Frame '{flags: 4'b1000, data: .d} &&& d == 8'h5a)
            hits++;
        if (message matches tagged Frame '{flags: .f, data: 8'h5a} &&& f[3] &&& !f[2])
            hits++;
        if (message matches tagged Pair .whole) $display("wrong tag bound");
        if (message matches tagged Frame .whole &&& whole.data == 8'h5a) begin
            result = whole.data;
            hits++;
        end
        $display("frame hits=%0d result=%h calls=%0d", hits, result, calls);

        message = tagged Pair '{8'h01, 8'h02};
        case (sample()) matches
            tagged Frame .f: $display("case frame");
            tagged Pair .p &&& p[0] == 8'h01: $display("case pair %h %h", p[0], p[1]);
            default: $display("case default");
        endcase
        result = message matches tagged Pair .q &&& q[1] == 8'h02 ? q[1] : 8'hff;
        $display("conditional=%h calls=%0d", result, calls);

        message = tagged Nested (tagged High (4'h9));
        case (message) matches
            tagged Nested (tagged Low .l): $display("nested low %h", l);
            tagged Nested (tagged High .h) &&& h > 4'h8: $display("nested high %h", h);
            default: $display("nested default");
        endcase

        message = tagged Empty;
        unique case (message) matches
            tagged Empty: $display("empty");
            tagged Frame .*: $display("frame");
        endcase
        $finish(0);
    end
endmodule
