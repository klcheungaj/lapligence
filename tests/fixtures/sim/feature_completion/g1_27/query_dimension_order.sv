// IEEE 1800-2009 20.7: dimensions are numbered slowest-varying first;
// intermediate typedefs are expanded before numbering. Packed and unpacked
// bounds, increment and size stay in declaration order for mixed ascending
// arrays, and a 1-bit bit-vector type still reports one dimension. The
// expected trace is an independent dimension-table oracle in
// tests/sim_g1_closure.rs.
module tb;
    typedef logic [8:1] Word;
    typedef struct packed { logic only; } bit_struct_t;

    logic [3:0][2:1] n [1:5][2:8];
    logic [7:0] fixed [7:4];
    logic [0:7] asc [0:1];
    Word ram [0:9];
    bit_struct_t bit_struct;
    logic scalar;
    bit scalar_two;
    integer i;

    initial begin
        $display("n dims=%0d unpacked=%0d", $dimensions(n), $unpacked_dimensions(n));
        for (i = 1; i <= 4; i = i + 1) begin
            $display("n[%0d]=%0d %0d %0d %0d %0d %0d", i,
                     $left(n, i), $right(n, i), $low(n, i), $high(n, i),
                     $increment(n, i), $size(n, i));
        end
        $display("fixed dims=%0d unpacked=%0d", $dimensions(fixed), $unpacked_dimensions(fixed));
        for (i = 1; i <= 2; i = i + 1) begin
            $display("fixed[%0d]=%0d %0d %0d %0d %0d %0d", i,
                     $left(fixed, i), $right(fixed, i), $low(fixed, i), $high(fixed, i),
                     $increment(fixed, i), $size(fixed, i));
        end
        $display("asc dims=%0d unpacked=%0d", $dimensions(asc), $unpacked_dimensions(asc));
        $display("asc[1]=%0d %0d %0d %0d %0d %0d",
                 $left(asc), $right(asc), $low(asc), $high(asc),
                 $increment(asc), $size(asc));
        $display("asc[2]=%0d %0d %0d %0d %0d %0d",
                 $left(asc, 2), $right(asc, 2), $low(asc, 2), $high(asc, 2),
                 $increment(asc, 2), $size(asc, 2));
        $display("scalar dims=%0d unpacked=%0d", $dimensions(scalar), $unpacked_dimensions(scalar));
        $display("scalar_two dims=%0d unpacked=%0d", $dimensions(scalar_two), $unpacked_dimensions(scalar_two));
        $display("bit_struct dims=%0d unpacked=%0d", $dimensions(bit_struct), $unpacked_dimensions(bit_struct));
        $display("Word size=%0d dims=%0d", $size(Word), $dimensions(Word));
        $display("ram dims=%0d unpacked=%0d size2=%0d left=%0d", $dimensions(ram), $unpacked_dimensions(ram), $size(ram, 2), $left(ram));
        $display("PASS query_dimension_order");
        $finish(0);
    end
endmodule
