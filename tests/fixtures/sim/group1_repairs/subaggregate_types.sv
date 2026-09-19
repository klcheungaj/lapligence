module tb;
    typedef struct { int x; } inner_t;
    typedef struct { inner_t part; int tag; } left_t;
    typedef struct { inner_t part; bit flag; } right_t;
    left_t a;
    right_t b;
    inner_t standalone;
    initial begin
        a.tag = 77;
        a.part.x = 1;
        b.part.x = 23;
        b.flag = 1;
        a.part = b.part;
        if (a.part.x != 23 || a.tag != 77) $fatal(1, "nested copy");
        standalone = a.part;
        if (standalone.x != 23) $fatal(1, "nested to whole");
        standalone.x = 45;
        b.part = standalone;
        if (b.part.x != 45 || b.flag != 1) $fatal(1, "whole to nested");
        a.part <= b.part;
        #1;
        if (a.part.x != 45 || a.tag != 77) $fatal(1, "nested NBA copy");
        $display("selected aggregate types passed");
        $finish(0);
    end
endmodule
