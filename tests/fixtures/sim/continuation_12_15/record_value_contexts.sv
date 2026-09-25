// SV 11.4.5 / 11.4.11 / 7.2: unpacked-structure parameters are values, and
// equality compares every member whether an operand is storage or a value.
module tb;
    typedef struct { logic [6:0] data; bit flag; logic [7:0] row[2]; } record_t;
    typedef struct { record_t child; logic [7:0] tail; } outer_t;
    localparam record_t A = '{data:7'h7f, flag:1, row:'{8'h10, 8'h11}};
    localparam record_t B = '{data:7'h00, flag:0, row:'{8'h10, 8'h12}};
    // Ambiguous selector: equal members survive, others take their default.
    localparam record_t MERGED = 1'bx ? A : B;
    localparam outer_t OUTER = '{A, 8'hc3};
    record_t a, b;
    logic unknown;

    function automatic record_t identity(input record_t value);
        return value;
    endfunction

    initial begin
        a = A;
        b = MERGED;
        if (a.data !== 7'h7f || a.flag !== 1 || a.row[0] !== 8'h10 || a.row[1] !== 8'h11)
            $fatal(1, "whole structure parameter read");
        if (b.data !== 7'bx || b.flag !== 0 || b.row[0] !== 8'hxx || b.row[1] !== 8'hxx)
            $fatal(1, "conditional structure parameter merge");
        if (OUTER.child.flag !== 1 || OUTER.child.row[1] !== 8'h11 || OUTER.tail !== 8'hc3)
            $fatal(1, "nested parameter member read");
        if (!(a === A) || a !== identity(A) || !(identity(a) == A) || a != A)
            $fatal(1, "storage and value operands compare equal");
        a.row[1] = 8'h12;
        if (a === A || !(a !== identity(A)) || a == A || !(a != A))
            $fatal(1, "a known member mismatch compares unequal");
        a.row[1] = 8'hx1;
        unknown = a == A;
        if (unknown !== 1'bx || (a === A) !== 0 || (a != A) !== 1'bx)
            $fatal(1, "an unknown member makes logical equality unknown");
        a.data = 7'h00;
        if ((a == A) !== 0)
            $fatal(1, "a known mismatch dominates an unknown member");
        $display("RECORD_VALUE_CONTEXTS_PASS");
        $finish(0);
    end
endmodule
