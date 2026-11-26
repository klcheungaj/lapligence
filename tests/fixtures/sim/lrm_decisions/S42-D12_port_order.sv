// Decision S42-D12: ports iterate in port order (vpiPortIndex), the order of
// the port list whatever the order of the declarations; io declarations of a
// subroutine in argument order. Regression: the runtime once sorted ports by
// the index of the catalog row before each one, which glibc's stable qsort
// hid and the Microsoft CRT's did not (the test builds with LLG_CO_DEBUG,
// whose sort reverses its input first).
//
// Build S42-D12_port_order.c as a VPI application (llg: static archive given
// to --dpi-lib; `task $port_order()` declared as a system task).
module ansi (output logic [3:0] zz, input logic aa, input logic [1:0] mm);
    logic hold = 1'b0;
    assign zz = {aa, mm, hold};
endmodule
module legacy (b, a, c);
    input a;
    output b;
    inout c;
    wire mid;
    assign mid = a;
    assign b = mid;
endmodule
module tb;
    wire pa = 1'b1, pb0, pb2, pc;
    logic [3:0] w4;
    logic [1:0] two = 2'b10;
    int cio = 1;
    logic [3:0] bo;
    ansi u1 (.zz(w4), .aa(pa), .mm(two));
    legacy u0 (pb0, pa, pc);
    legacy u2 (.c(pc), .a(pa), .b(pb2));
    task st(input int zz, output logic [3:0] aa, inout int mm);
        int loc;
        loc = zz + 1;
        aa = zz[3:0];
        mm = mm + loc;
    endtask
    initial begin
        st(5, bo, cio);
        #1;
        $port_order;
        $finish;
    end
endmodule
