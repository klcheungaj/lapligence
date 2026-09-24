module tb;
    typedef struct packed { logic [7:0] hi; logic [7:0] lo; } pair_t;
    typedef union packed { logic [15:0] word; pair_t halves; } union_t;
    localparam logic [7:0] PARAM = 8'hA5;
    localparam pair_t const_pair = '{hi:8'h12, lo:8'h34};
    localparam union_t const_union = union_t'(16'hB5C6);
    localparam logic const_pred = PARAM == 8'hA5;
    pair_t runtime_pair = '{hi:8'h21, lo:8'h43};
    union_t runtime_union = union_t'(16'hC5D6);
    logic runtime_pred = PARAM == 8'hA5;

    initial begin : check
        static pair_t static_pair = '{hi:8'h31, lo:8'h53};
        static union_t static_union = union_t'(16'hD5E6);
        static logic static_pred = PARAM == 8'hA5;
        automatic pair_t automatic_pair = '{hi:8'h41, lo:8'h63};
        automatic union_t automatic_union = union_t'(16'hE5F6);
        automatic logic automatic_pred = PARAM == 8'hA5;
        if (16'(const_pair) !== 16'h1234 || 16'(const_union) !== 16'hB5C6 || !const_pred ||
            runtime_pair.hi !== 8'h21 || runtime_union.word !== 16'hC5D6 || !runtime_pred ||
            static_pair.hi !== 8'h31 || static_union.word !== 16'hD5E6 || !static_pred ||
            automatic_pair.hi !== 8'h41 || automatic_union.word !== 16'hE5F6 || !automatic_pred)
            $fatal(1, "typed initializer matrix mismatch");
        $display("const=%h,%h,%b runtime=%h,%h,%b static=%h,%h,%b auto=%h,%h,%b",
                 16'(const_pair), 16'(const_union), const_pred,
                 runtime_pair, runtime_union.word, runtime_pred,
                 static_pair, static_union.word, static_pred,
                 automatic_pair, automatic_union.word, automatic_pred);
        $finish(0);
    end
endmodule
