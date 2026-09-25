module tb;
    typedef struct { int a; int b; } record_t;
    record_t values [1:0];
    int key;
    initial if (key inside {values}) $display("invalid aggregate element");
endmodule
