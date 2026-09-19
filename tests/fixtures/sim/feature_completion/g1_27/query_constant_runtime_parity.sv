// IEEE 1800-2009 20.6-20.8: array/data query metadata must be identical when
// folded at elaboration and when read from a runtime variable. Each line
// prints the localparam result and the runtime result side by side.
module tb;
    typedef struct packed { logic [3:0] hi; logic [3:0] lo; } pair_t;

    logic [7:0] arr [7:4];
    logic [3:0][2:1] n [1:5][2:8];
    pair_t p;

    parameter integer P = 7;
    parameter integer UB = $;

    localparam integer D_ARR = $dimensions(arr);
    localparam integer UD_ARR = $unpacked_dimensions(arr);
    localparam integer L_ARR = $left(arr);
    localparam integer R_ARR = $right(arr);
    localparam integer LO_ARR = $low(arr);
    localparam integer HI_ARR = $high(arr);
    localparam integer INC_ARR = $increment(arr);
    localparam integer SZ_ARR = $size(arr);
    localparam integer BITS_ARR = $bits(arr);
    localparam integer D_N = $dimensions(n);
    localparam integer SZ_N2 = $size(n, 2);
    localparam integer BITS_P = $bits(p);
    localparam integer CL = $clog2(P);
    localparam integer ISUN = $isunbounded(P);
    localparam integer ISUB = $isunbounded(UB);

    initial begin
        $display("arr dims=%0d+%0d unpacked=%0d+%0d left=%0d+%0d right=%0d+%0d low=%0d+%0d high=%0d+%0d inc=%0d+%0d size=%0d+%0d bits=%0d+%0d",
                 D_ARR, $dimensions(arr), UD_ARR, $unpacked_dimensions(arr),
                 L_ARR, $left(arr), R_ARR, $right(arr),
                 LO_ARR, $low(arr), HI_ARR, $high(arr),
                 INC_ARR, $increment(arr), SZ_ARR, $size(arr),
                 BITS_ARR, $bits(arr));
        $display("n dims=%0d+%0d size2=%0d+%0d", D_N, $dimensions(n), SZ_N2, $size(n, 2));
        $display("p bits=%0d+%0d", BITS_P, $bits(p));
        $display("clog2=%0d+%0d", CL, $clog2(P));
        $display("isunbounded=%0d+%0d unbounded=%0d+%0d", ISUN, $isunbounded(P), ISUB, $isunbounded(UB));
        $display("PASS query_constant_runtime_parity");
        $finish(0);
    end
endmodule
