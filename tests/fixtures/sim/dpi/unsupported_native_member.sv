module tb;
    typedef struct { int count; real scale; } sample_t;
    import "DPI-C" sample_sum = function int sample_sum(input sample_t value);

    sample_t sample;

    initial begin
        sample.count = 1;
        sample.scale = 0.5;
        $display("%0d", sample_sum(sample));
        $finish;
    end
endmodule
