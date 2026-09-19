module tb;
    typedef struct packed { logic [7:0] value; } pair_t;
    pair_t source;
    task automatic invalid(const ref pair_t target); target.value = 8'd1; endtask
    initial invalid(source);
endmodule
