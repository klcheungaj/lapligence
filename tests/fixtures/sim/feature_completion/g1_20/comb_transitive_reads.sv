// llg-test-fixture: tests/fixtures/sim/feature_completion/g1_20/comb_transitive_reads.sv
// G1-20 comb_transitive_function_reads: an always_comb process is sensitive to
// a global read performed inside a called function, including fixed-array
// element and aggregate-member leaves. The call counters are declaration
// initializers so the comb processes remain their only procedural writers.
module tb;
    logic [3:0] mem [0:1];
    typedef struct { logic [3:0] a; logic [3:0] b; } u_t;
    u_t u;
    logic [3:0] from_mem;
    logic [3:0] from_member;
    integer mem_reads = 0;
    integer member_reads = 0;

    function automatic logic [3:0] read_mem(input int idx);
        mem_reads = mem_reads + 1;
        read_mem = mem[idx];
    endfunction

    function automatic logic [3:0] read_member();
        member_reads = member_reads + 1;
        read_member = u.a;
    endfunction

    always_comb from_mem = read_mem(1);
    always_comb from_member = read_member();

    initial begin
        mem[0] = 4'h0;
        mem[1] = 4'h1;
        u.a = 4'h2;
        u.b = 4'h3;
        #1 $display("t1 mem=%h member=%h reads=%0d/%0d", from_mem, from_member,
                    mem_reads, member_reads);
        mem[1] = 4'h5;
        #1 $display("t2 mem=%h member=%h reads=%0d/%0d", from_mem, from_member,
                    mem_reads, member_reads);
        u.a = 4'h7;
        #1 $display("t3 mem=%h member=%h reads=%0d/%0d", from_mem, from_member,
                    mem_reads, member_reads);
        // Neither write touches a read slot: the comb processes must not rerun.
        u.b = 4'h9;
        mem[0] = 4'ha;
        #1 $display("t4 mem=%h member=%h reads=%0d/%0d", from_mem, from_member,
                    mem_reads, member_reads);
        $finish(0);
    end
endmodule
