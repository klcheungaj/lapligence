module tb;
    typedef struct { int count; real scale; } sample_t;
    export "DPI-C" function sample_sum;

    function int sample_sum(input sample_t value);
        return value.count;
    endfunction

    initial $finish;
endmodule
