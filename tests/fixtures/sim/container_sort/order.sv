// IEEE 1800-2009 7.12.2: sort/rsort on queues and dynamic arrays, with and
// without a `with` key. Expected lines are fixed by the test, not computed here.
`define SHOWD(tag, arr) begin $write(tag); foreach (arr[i]) $write(" %0d", arr[i]); $display; end
`define SHOWB(tag, arr) begin $write(tag); foreach (arr[i]) $write(" %b", arr[i]); $display; end
`define SHOWH(tag, arr) begin $write(tag); foreach (arr[i]) $write(" %h", arr[i]); $display; end

module tb;
    int q[$];
    int d[];
    bit [7:0] tagged_q[$];
    bit [7:0] tagged_d[];
    bit [7:0] u8[$];
    logic [3:0] xq[$];
    logic [3:0] xd[];
    int kq[$];
    logic signed [127:0] wide_q[$];
    logic [99:0] wide_u[];

    initial begin
        // Plain sort with duplicates and negatives.
        q = '{5, 3, 9, 3, 1, 9, 0, -4};
        q.sort();
        `SHOWD("q.sort", q)
        q = '{5, 3, 9, 3, 1, 9, 0, -4};
        q.rsort();
        `SHOWD("q.rsort", q)
        d = new[8];
        d = '{5, 3, 9, 3, 1, 9, 0, -4};
        d.sort() with (item);
        `SHOWD("d.sort", d)
        d = '{5, 3, 9, 3, 1, 9, 0, -4};
        d.rsort() with (item);
        `SHOWD("d.rsort", d)

        // The key is the high nibble; equal keys keep their original order
        // for both directions.
        tagged_q = '{8'h21, 8'h12, 8'h23, 8'h11, 8'h22, 8'h13};
        tagged_q.sort() with (item[7:4]);
        `SHOWH("tq.sort", tagged_q)
        tagged_q = '{8'h21, 8'h12, 8'h23, 8'h11, 8'h22, 8'h13};
        tagged_q.rsort() with (item[7:4]);
        `SHOWH("tq.rsort", tagged_q)
        tagged_d = new[6];
        tagged_d = '{8'h21, 8'h12, 8'h23, 8'h11, 8'h22, 8'h13};
        tagged_d.sort() with (item[7:4]);
        `SHOWH("td.sort", tagged_d)
        tagged_d = '{8'h21, 8'h12, 8'h23, 8'h11, 8'h22, 8'h13};
        tagged_d.rsort() with (item[7:4]);
        `SHOWH("td.rsort", tagged_d)

        // Unsigned elements, then the same bytes keyed as signed.
        u8 = '{8'd200, 8'd5, 8'd130, 8'd127, 8'd128};
        u8.sort();
        `SHOWD("u8.sort", u8)
        u8 = '{8'd200, 8'd5, 8'd130, 8'd127, 8'd128};
        u8.sort() with ($signed(item));
        `SHOWD("u8.signed_sort", u8)
        u8 = '{8'd200, 8'd5, 8'd130, 8'd127, 8'd128};
        u8.rsort() with ($signed(item));
        `SHOWD("u8.signed_rsort", u8)

        // Elements whose key has an X/Z bit stay in place and no element
        // crosses them.
        xq = '{4'd5, 4'd2, 4'bx, 4'd1, 4'd0, 4'bz, 4'd3, 4'd9};
        xq.sort();
        `SHOWH("xq.sort", xq)
        xq = '{4'd5, 4'd2, 4'bx, 4'd1, 4'd0, 4'bz, 4'd3, 4'd9};
        xq.rsort();
        `SHOWH("xq.rsort", xq)
        xd = new[8];
        xd = '{4'd5, 4'd2, 4'b1x00, 4'd1, 4'd0, 4'b0z11, 4'd3, 4'd9};
        xd.sort();
        `SHOWB("xd.sort", xd)
        xd = '{4'd5, 4'd2, 4'b1x00, 4'd1, 4'd0, 4'b0z11, 4'd3, 4'd9};
        xd.rsort();
        `SHOWB("xd.rsort", xd)
        kq = '{6, 4, 5, 2, 9, 1, 8, 0};
        kq.sort() with ((item % 2 == 0) ? item : 32'sbx);
        `SHOWD("kq.xkey_sort", kq)
        kq = '{6, 4, 5, 2, 9, 1, 8, 0};
        kq.rsort() with ((item % 2 == 0) ? item : 32'sbx);
        `SHOWD("kq.xkey_rsort", kq)

        // Keys wider than a machine word, signed and unsigned.
        wide_q = '{128'sd3, -128'sd7, 128'sd1 << 100, -(128'sd1 << 100), 128'sd0};
        wide_q.sort();
        $write("wide_q.sort");
        foreach (wide_q[i]) $write(" %0d", wide_q[i] >>> 96);
        $write(" |");
        foreach (wide_q[i]) $write(" %0d", wide_q[i][7:0]);
        $display;
        wide_u = new[4];
        wide_u = '{100'h8_0000_0000_0000_0000_0000_0001, 100'd5,
                   100'h8_0000_0000_0000_0000_0000_0000,
                   100'h1_0000_0000_0000_0000_0000_0000};
        wide_u.rsort();
        `SHOWH("wide_u.rsort", wide_u)
        wide_u.sort();
        `SHOWH("wide_u.sort", wide_u)

        // item.index is the position before the sort.
        q = '{10, 20, 30, 40};
        q.rsort() with (item.index);
        `SHOWD("q.rsort_index", q)
        q.sort() with (item.index);
        `SHOWD("q.sort_index", q)
        q = '{40, 10, 30, 20};
        q.sort() with (item - item.index * 100);
        `SHOWD("q.sort_index_mix", q)

        // Empty and single-element receivers are left alone.
        q.delete();
        q.sort();
        q.rsort() with (item);
        $display("empty %0d", q.size());
        q = '{7};
        q.sort();
        `SHOWD("single", q)
    end
endmodule
