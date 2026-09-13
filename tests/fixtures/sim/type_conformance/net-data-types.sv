module tb;
    typedef logic signed [64:0] vector_t;
    typedef struct packed signed { logic [32:0] hi; logic [31:0] lo; } packed_t;
    typedef enum logic signed [64:0] { ZERO=0, NEGATIVE=-3 } enum_t;
    logic signed [64:0] source;
    wire integer i=source;
    tri time t=source;
    uwire vector_t vector_value=source;
    wire packed_t packed_value=source;
    wire enum_t enum_value=enum_t'(source);
    initial begin
        if ($bits(i) != 32 || $bits(t) != 64 || $bits(vector_value) != 65 ||
            $bits(packed_value) != 65 || $bits(enum_value) != 65) $display("FAIL net widths");
        source=-3; #1;
        if (i !== -3 || t !== 64'hfffffffffffffffd || vector_value !== source ||
            packed_value !== source || enum_value !== source) $display("FAIL net numeric values");
        if ((i<0) !== 1 || (t<0) !== 0 || (vector_value<0) !== 1 ||
            (packed_value<0) !== 1 || (enum_value<0) !== 1) $display("FAIL net signedness");
        source={1'bx,32'hz0000001,32'h10xz0101}; #1;
        if (i !== source[31:0] || t !== source[63:0] || vector_value !== source ||
            packed_value !== source || enum_value !== source || int'(i) !== 32'h10000101)
            $display("FAIL net unknowns");
        source='z; #1;
        if (i !== 'z || t !== 'z || vector_value !== 'z ||
            packed_value !== 'z || enum_value !== 'z) $display("FAIL typed net release");
        $display("PASS net datatypes"); $finish(0);
    end
endmodule
