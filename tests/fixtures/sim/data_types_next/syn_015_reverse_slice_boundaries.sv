// llg-test-fixture: tests/fixtures/sim/data_types_next/syn_015_reverse_slice_boundaries.sv
// IEEE 1800-2009 §§11.4.14.2, 11.4.14.3: `<<` slices the stream into blocks
// "starting with the right-most bit", the left-most block keeps the remaining
// bits, and the blocks are reversed with the order of bits within each block
// preserved; an unpack consumes the source's left-most bits. Every consumed
// width 1..64 under slice sizes on both sides of each block boundary, with
// X and Z bits, through a runtime `with` target (`unpack_case`), and whole
// descriptor-backed arrays whose cells are slice-reversed one by one
// (`copy_case`). Every oracle uses only bit selects and integer arithmetic.
module unpack_case #(parameter int S = 1)(output bit done);
    logic t [0:63];
    logic [63:0] src;
    int n, i, k, o, len, dest, errors;

    initial begin
        errors = 0;
        src = {32'hF0E1_D2C3, 8'b1x0z_01zx, 24'hA5_9687} ^ (64'(S) << 40);
        for (n = 1; n <= 64; n++) begin
            for (i = 0; i < 64; i++) t[i] = 1'b0;
            {<<S{t with [0 +: n]}} = src;
            // Consumed bit i is src[64 - n + i]; it lands at stream bit
            // `dest`, and t[0] takes the stream's left-most bit.
            for (i = 0; i < n; i++) begin
                k = i / S;
                o = i % S;
                len = n - k * S < S ? n - k * S : S;
                dest = n - k * S - len + o;
                if (t[n - 1 - dest] !== src[64 - n + i]) errors++;
            end
            for (i = n; i < 64; i++)
                if (t[i] !== 1'b0) errors++;
        end
        if (errors != 0) $display("unpack S=%0d: %0d wrong bits", S, errors);
        done = errors == 0;
    end
endmodule

// Cell j of the target is cell N-1-j of the source with its S-bit blocks
// reversed (S divides W): source bit i of a cell lands at bit
// W - (i / S + 1) * S + i % S. Unwritten cells stay X.
module copy_case #(parameter int W = 8, parameter int S = 1)(output bit done);
    localparam int N = 8192;
    logic [W-1:0] src [0:N-1];
    logic [W-1:0] tgt [0:N-1];
    logic [W-1:0] expected;
    int picks [6] = '{0, 1, N / 2 - 1, N / 2, N - 2, N - 1};
    int p, j, i, errors;

    initial begin
        errors = 0;
        for (p = 0; p < 6; p++)
            src[picks[p]] = W'({64'hF0E1_D2C3_B4A5_9687} >> (5 * p)) ^ W'(p);
        src[N / 2] = W'({2{32'b1x0z_01zx_1100_zx01_0z1x_10x0_z1zz_0x10}});
        tgt = {<<S{src}};
        for (p = 0; p < 6; p++) begin
            j = N - 1 - picks[p];
            for (i = 0; i < W; i++) expected[W - (i / S + 1) * S + i % S] = src[picks[p]][i];
            if (tgt[j] !== expected) errors++;
        end
        if (tgt[2] !== 'x || tgt[N - 3] !== 'x) errors++;
        if (errors != 0) $display("copy W=%0d S=%0d: %0d wrong cells", W, S, errors);
        done = errors == 0;
    end
endmodule

module tb;
    localparam int UNPACK_SLICES = 15;
    localparam int SLICES [UNPACK_SLICES] = '{1, 2, 3, 4, 5, 7, 8, 9, 13, 16, 31, 32, 33, 63, 64};
    localparam int COPIES = 12;
    localparam int COPY_W [COPIES] = '{8, 8, 8, 6, 6, 5, 16, 64, 64, 64, 63, 63};
    localparam int COPY_S [COPIES] = '{1, 2, 4, 2, 3, 1, 4, 1, 8, 32, 7, 21};
    bit [UNPACK_SLICES-1:0] unpacked;
    bit [COPIES-1:0] copied;

    for (genvar g = 0; g < UNPACK_SLICES; g++) begin : unpack
        unpack_case #(.S(SLICES[g])) check(.done(unpacked[g]));
    end
    for (genvar g = 0; g < COPIES; g++) begin : copy
        copy_case #(.W(COPY_W[g]), .S(COPY_S[g])) check(.done(copied[g]));
    end

    initial begin
        #1;
        if (&unpacked && &copied) $display("SYN015_REVERSE_SLICE_BOUNDARIES_PASS");
        else $display("SYN015_REVERSE_SLICE_BOUNDARIES_FAIL %b %b", unpacked, copied);
        $finish(0);
    end
endmodule
