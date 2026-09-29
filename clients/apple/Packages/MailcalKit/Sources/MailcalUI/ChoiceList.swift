// A single choice among a few long options: a radio group on macOS, a column of rows with a mark
// on iOS.
//
// Not `radioPickerStyle` on iOS, where an inline picker outside a Form is a wheel: it cuts a long
// option to one line, and it draws its first row as chosen when nothing is, which for "where does
// your endpoint run?" shows a declaration the person never made.

import SwiftUI

struct ChoiceList<Value: Hashable>: View {
    let title: String
    @Binding var selection: Value
    let options: [(value: Value, label: String)]

    var body: some View {
        #if os(macOS)
        Picker(title, selection: $selection) {
            ForEach(options, id: \.value) { option in
                Text(option.label).tag(option.value)
            }
        }
        .pickerStyle(.radioGroup)
        .labelsHidden()
        #else
        VStack(alignment: .leading, spacing: 10) {
            ForEach(options, id: \.value) { option in
                Button {
                    selection = option.value
                } label: {
                    HStack(alignment: .firstTextBaseline, spacing: 10) {
                        Image(systemName: selection == option.value ? "checkmark.circle.fill" : "circle")
                            .foregroundStyle(selection == option.value ? Color.accentColor : Color.secondary)
                        Text(option.label)
                            .foregroundStyle(.primary)
                            .multilineTextAlignment(.leading)
                            .fixedSize(horizontal: false, vertical: true)
                    }
                    .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .accessibilityAddTraits(selection == option.value ? .isSelected : [])
            }
        }
        #endif
    }
}
