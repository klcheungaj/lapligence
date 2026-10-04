// IEEE 1800-2009 29.3.1, 29.8 (IEEE 1364-2001 8.1.1, 8.6): UDP terminals are
// scalar, but the connected expressions may select one bit of a larger object.
// Inputs read bit-, part- and indexed selects, unpacked and packed array
// elements, structure members, a descriptor-backed array larger than the
// packed limit, hierarchical names, constants, a function call and a
// conditional. Outputs drive a vector bit, unpacked net elements, a net-array
// cell bit and a net inside another instance.
`timescale 1ns/1ns
primitive and2(y, a, b);
    output y;
    input a, b;
    table
        0 ? : 0;
        ? 0 : 0;
        1 1 : 1;
    endtable
endprimitive

primitive inv(y, a);
    output y;
    input a;
    table
        0 : 1;
        1 : 0;
    endtable
endprimitive

module leaf(output wire o, input wire i);
    wire inner;
    inv g(o, i);
endmodule

module tb;
    localparam P = 1'b1;
    localparam integer K = 5;
    typedef struct packed {
        logic f;
        logic [2:0] g;
    } s_t;

    reg [7:0] v;
    reg m [0:3];
    reg [3:0] mem [0:3];
    reg [3:0][1:0] pk;
    s_t st;
    reg [7:0] big [0:199999];
    wire lo;
    wire [3:0] yv;
    wire yn [0:2];
    wire [1:0] cells [0:3];
    wire y_bit, y_part, y_idx, y_mem, y_pk, y_st, y_big, y_hier, y_fn, y_cond, y_z;

    function automatic logic flip(input logic value);
        return ~value;
    endfunction

    leaf l(.o(lo), .i(v[0]));
    and2 g_bit(y_bit, v[3], v[K]);
    and2 g_part(y_part, v[6:6], v[7 -: 1]);
    and2 g_idx(y_idx, v[K +: 1], P);
    and2 g_mem(y_mem, mem[2][1], m[3]);
    and2 g_pk(y_pk, pk[2][1], pk[0][0]);
    and2 g_st(y_st, st.f, st.g[2]);
    and2 g_big(y_big, big[123456][7], big[199999][0]);
    inv g_hier(y_hier, tb.l.o);
    inv g_inner(l.inner, v[1]);
    and2 g_fn(y_fn, ~v[2], flip(v[4]));
    and2 g_cond(y_cond, v[0] ? v[1] : v[2], 1'b1);
    and2 g_z(y_z, 1'bz, v[0]);
    and2 g_vec(yv[2], v[1], v[2]);
    inv g_n0(yn[0], v[2]);
    and2 g_n1(yn[1], v[6], v[7]);
    inv g_cell(cells[2][1], v[3]);

    task show(input [7:0] label);
        $display("%s bit=%b part=%b idx=%b mem=%b pk=%b st=%b big=%b hier=%b/%b inner=%b fn=%b cond=%b z=%b yv=%b yn=%b%b%b cell=%b",
                 label, y_bit, y_part, y_idx, y_mem, y_pk, y_st, y_big, y_hier, lo, l.inner,
                 y_fn, y_cond, y_z, yv, yn[0], yn[1], yn[2], cells[2]);
    endtask

    initial begin
        v = 8'b1010_0110;
        m[3] = 1'b1;
        mem[2] = 4'b0010;
        pk = '0;
        pk[2] = 2'b10;
        pk[0] = 2'b01;
        st = 4'b1_100;
        big[123456] = 8'h80;
        big[199999] = 8'h01;
        #1 show("A");
        v = 8'b1110_1001;
        m[3] = 1'b0;
        mem[2] = 4'b0000;
        pk[2] = 2'b01;
        pk[0] = 2'b00;
        st = 4'b0_011;
        big[123456] = 8'h7f;
        #1 show("B");
        $finish(0);
    end
endmodule
