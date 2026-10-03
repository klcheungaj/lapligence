// IEEE 1800-2009 12.5.1, 12.6.1, 11.9: an uninitialized tagged union has
// undefined (X) tag bits. Ordinary case and if-matches compare tags exactly,
// so no tag matches; casex treats the X tag bits as wildcards, so the first
// tagged item whose payload pattern also matches is taken.
typedef union tagged {
    void Empty;
    logic [7:0] Data;
    logic [3:0] Small;
} message_t;

module tb;
    message_t message;

    initial begin
        case (message) matches
            tagged Empty: $display("case empty");
            tagged Data .*: $display("case data");
            default: $display("case default");
        endcase
        casex (message) matches
            tagged Data .*: $display("casex data");
            default: $display("casex default");
        endcase
        if (message matches tagged Small .*) $display("if small");
        else $display("if none");
        message = tagged Small (4'h3);
        casez (message) matches
            tagged Data .*: $display("casez data");
            tagged Small 4'b00?1: $display("casez small");
            default: $display("casez default");
        endcase
        $finish(0);
    end
endmodule
