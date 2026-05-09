using System;
using System.Runtime.InteropServices;

namespace OxVba.TestEventServer
{
    [ComVisible(true)]
    [Guid("E2A30001-0001-0001-0001-000000000002")]
    [InterfaceType(ComInterfaceType.InterfaceIsIDispatch)]
    public interface IOxVbaTestEventServerEvents
    {
        [DispId(1)]
        void OnSimpleEvent();

        [DispId(2)]
        void OnValueChanged(int value);

        [DispId(3)]
        void OnPairChanged(int a, int b);
    }

    [ComVisible(true)]
    [Guid("E2A30001-0001-0001-0001-000000000003")]
    public interface IOxVbaTestEventServer
    {
        [DispId(101)]
        void FireSimpleEvent();

        [DispId(102)]
        void FireValueChanged(int value);

        [DispId(103)]
        void FirePairChanged(int a);

        [DispId(104)]
        int Ping();

        [DispId(105)]
        int SumPair(int lhs, int rhs);

        [DispId(106)]
        string DescribeArrayShape(object value);

        [DispId(107)]
        bool IsSelf(object value);

        [DispId(108)]
        object ReturnLongArray();

        [DispId(109)]
        object ReturnSelfArray();

        [DispId(110)]
        object ReturnSignedByteObject();

        [DispId(111)]
        object ReturnUnsignedWordObject();

        [DispId(112)]
        object ReturnUnsignedLongObject();

        [DispId(113)]
        object ReturnUnsignedHyperObject();

        [DispId(114)]
        object ReturnDecimalObject();

        [DispId(115)]
        object ReturnSignedByteArray();

        [DispId(116)]
        object ReturnUnsignedWordArray();

        [DispId(117)]
        object ReturnUnsignedLongArray();

        [DispId(118)]
        object ReturnUnsignedHyperArray();

        [DispId(119)]
        object ReturnDecimalArray();

        [DispId(120)]
        object ReturnSelfObject();

        [DispId(121)]
        object ReturnRecordObject();

        [DispId(122)]
        object ReturnRecordArray();
    }

    [ComVisible(true)]
    [Guid("E2A30001-0001-0001-0001-000000000005")]
    public struct TestRecord
    {
        public int Number;
    }

    [ComVisible(true)]
    [Guid("E2A30001-0001-0001-0001-000000000004")]
    [ClassInterface(ClassInterfaceType.None)]
    [ComSourceInterfaces(typeof(IOxVbaTestEventServerEvents))]
    [ProgId("OxVba.TestEventServer")]
    public class TestEventServer : IOxVbaTestEventServer
    {
        public delegate void OnSimpleEventHandler();
        public delegate void OnValueChangedHandler(int value);
        public delegate void OnPairChangedHandler(int a, int b);

        public event OnSimpleEventHandler OnSimpleEvent;
        public event OnValueChangedHandler OnValueChanged;
        public event OnPairChangedHandler OnPairChanged;

        public void FireSimpleEvent()
        {
            OnSimpleEvent?.Invoke();
        }

        public void FireValueChanged(int value)
        {
            OnValueChanged?.Invoke(value);
        }

        public void FirePairChanged(int a)
        {
            OnPairChanged?.Invoke(a, a + 1);
        }

        public int Ping()
        {
            return 42;
        }

        public int SumPair(int lhs, int rhs)
        {
            return lhs + rhs;
        }

        public string DescribeArrayShape(object value)
        {
            if (!(value is Array array))
            {
                return "not-array";
            }

            int lowerBound = array.GetLowerBound(0);
            int upperBound = array.GetUpperBound(0);
            object first = array.Length == 0 ? null : array.GetValue(lowerBound);
            return string.Format(
                "rank={0};len={1};lb={2};ub={3};first={4}",
                array.Rank,
                array.Length,
                lowerBound,
                upperBound,
                FormatValue(first)
            );
        }

        public bool IsSelf(object value)
        {
            return ReferenceEquals(this, value);
        }

        public object ReturnLongArray()
        {
            return new object[] { 4, 5, 6 };
        }

        public object ReturnSelfArray()
        {
            return new object[] { this };
        }

        public object ReturnSignedByteObject()
        {
            return (sbyte)-5;
        }

        public object ReturnUnsignedWordObject()
        {
            return (ushort)65000;
        }

        public object ReturnUnsignedLongObject()
        {
            return (uint)4000000000;
        }

        public object ReturnUnsignedHyperObject()
        {
            return (ulong)9000000000;
        }

        public object ReturnDecimalObject()
        {
            return -123.45m;
        }

        public object ReturnSignedByteArray()
        {
            return new sbyte[] { -5, 0, 120 };
        }

        public object ReturnUnsignedWordArray()
        {
            return new ushort[] { 12, 4096, 65000 };
        }

        public object ReturnUnsignedLongArray()
        {
            return new uint[] { 12, 4096, 4000000000 };
        }

        public object ReturnUnsignedHyperArray()
        {
            return new ulong[] { 12, 4096, 9000000000 };
        }

        public object ReturnDecimalArray()
        {
            return new decimal[] { 123.45m, -4.25m, 321m };
        }

        public object ReturnSelfObject()
        {
            return this;
        }

        public object ReturnRecordObject()
        {
            return new TestRecord { Number = 123 };
        }

        public object ReturnRecordArray()
        {
            return new TestRecord[] { new TestRecord { Number = 123 } };
        }

        private static string FormatValue(object value)
        {
            if (value == null)
            {
                return "<null>";
            }

            if (value is TestEventServer)
            {
                return "<self>";
            }

            return Convert.ToString(value, System.Globalization.CultureInfo.InvariantCulture);
        }
    }
}
