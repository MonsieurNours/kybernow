// Accueil : vos ordinateurs, ajout manuel, connexion.

import SwiftUI

struct HomeView: View {
    @EnvironmentObject var app: AppModel
    @State private var adding = false

    var body: some View {
        ZStack(alignment: .topTrailing) {
            Theme.background.ignoresSafeArea()
            ScrollView {
                VStack(alignment: .leading, spacing: 26) {
                    header
                    if app.store.hosts.isEmpty {
                        empty
                    } else {
                        LazyVGrid(columns: [GridItem(.adaptive(minimum: 380), spacing: 18)], spacing: 18) {
                            ForEach(app.store.hosts) { host in
                                HostCard(host: host)
                            }
                            addTile
                        }
                    }
                }
                .padding(40)
            }
            if let toast = app.toast {
                Toast(text: toast).padding(24)
                    .transition(.move(edge: .top).combined(with: .opacity))
            }
        }
        .sheet(isPresented: $adding) { AddHostSheet() }
        .preferredColorScheme(.dark)
        .frame(minWidth: 860, minHeight: 560)
    }

    private var header: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack(spacing: 12) {
                Mark(size: 44)
                (Text("Kyber").foregroundStyle(Theme.text) + Text("Now").foregroundStyle(Theme.accent))
                    .font(Theme.display(26))
            }
            Text("Sur quel ordinateur allez-vous ?")
                .font(Theme.display(40))
                .foregroundStyle(Theme.text)
            Text("Ajoutez un ordinateur équipé de KyberNow avec son adresse et le port de sa console.")
                .font(.system(size: 15))
                .foregroundStyle(Theme.muted)
        }
    }

    private var empty: some View {
        VStack(alignment: .leading, spacing: 14) {
            Text("Aucun ordinateur pour l'instant.").foregroundStyle(Theme.muted)
            Button("Ajouter un ordinateur") { adding = true }.buttonStyle(PrimaryButton())
        }
    }

    private var addTile: some View {
        Button { adding = true } label: {
            VStack(alignment: .leading, spacing: 6) {
                Label("Ajouter un ordinateur", systemImage: "plus")
                    .font(.system(size: 16, weight: .semibold))
                    .foregroundStyle(Theme.text)
                Text("Par adresse et port de la console")
                    .font(.system(size: 13)).foregroundStyle(Theme.muted)
            }
            .frame(maxWidth: .infinity, minHeight: 120, alignment: .leading)
            .padding(20)
            .overlay(RoundedRectangle(cornerRadius: 16).stroke(Theme.line, style: StrokeStyle(lineWidth: 1, dash: [5])))
        }
        .buttonStyle(.plain)
    }
}

struct HostCard: View {
    @EnvironmentObject var app: AppModel
    let host: Host

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            HStack {
                Text(host.name).font(Theme.display(26)).foregroundStyle(Theme.text)
                Spacer()
                Menu {
                    Button("Oublier cet ordinateur", role: .destructive) { app.store.remove(host) }
                } label: {
                    Image(systemName: "ellipsis.circle").foregroundStyle(Theme.muted)
                }
                .menuStyle(.borderlessButton)
                .frame(width: 30)
            }
            Text("\(host.address) · console \(host.consolePort)\(host.streamPort.map { " · flux \($0)" } ?? "")")
                .font(.system(size: 13, design: .monospaced))
                .foregroundStyle(Theme.muted)
            Picker("Profil", selection: Binding(
                get: { host.profile },
                set: { var h = host; h.profile = $0; app.store.update(h) }
            )) {
                ForEach(Profile.allCases) { Text($0.label).tag($0) }
            }
            .pickerStyle(.segmented)
            .labelsHidden()
            .frame(width: 260)
            Text(host.profile.hint).font(.system(size: 13)).foregroundStyle(Theme.muted)
            HStack(spacing: 14) {
                Button {
                    app.connect(host)
                } label: {
                    Label("Se connecter", systemImage: "arrow.right")
                }
                .buttonStyle(PrimaryButton())
                .disabled(app.session != nil)
                if let last = host.lastSession {
                    Text("Dernière session : \(last.formatted(.relative(presentation: .named)))")
                        .font(.system(size: 12)).foregroundStyle(Theme.muted)
                }
            }
        }
        .padding(24)
        .background(Theme.surface, in: RoundedRectangle(cornerRadius: 18))
        .overlay(RoundedRectangle(cornerRadius: 18).stroke(Theme.line))
    }
}

struct AddHostSheet: View {
    @EnvironmentObject var app: AppModel
    @Environment(\.dismiss) private var dismiss
    @State private var name = ""
    @State private var address = ""
    @State private var port = ""
    @State private var username = ""
    @State private var password = ""

    private var valid: Bool {
        !address.trimmingCharacters(in: .whitespaces).isEmpty && UInt16(port) != nil && !username.isEmpty && !password.isEmpty
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 18) {
            Text("Ajouter un ordinateur").font(Theme.display(26)).foregroundStyle(Theme.text)
            Text("Le port de la console s'affiche à la fin de l'installation sur l'ordinateur. Le compte est celui créé pendant l'installation.")
                .font(.system(size: 13)).foregroundStyle(Theme.muted)
                .fixedSize(horizontal: false, vertical: true)
            Form {
                TextField("Nom", text: $name, prompt: Text("PC du salon"))
                TextField("Adresse", text: $address, prompt: Text("192.168.1.100 ou pc.local"))
                TextField("Port de la console", text: $port, prompt: Text("37462"))
                TextField("Identifiant", text: $username)
                SecureField("Mot de passe", text: $password)
            }
            .formStyle(.grouped)
            Text("Le mot de passe est gardé dans le Trousseau de ce Mac.")
                .font(.system(size: 12)).foregroundStyle(Theme.muted)
            HStack {
                Spacer()
                Button("Annuler") { dismiss() }.buttonStyle(GhostButton())
                Button("Ajouter") {
                    let a = address.trimmingCharacters(in: .whitespaces)
                    app.store.add(Host(name: name.isEmpty ? a : name, address: a, consolePort: UInt16(port)!, username: username), password: password)
                    dismiss()
                }
                .buttonStyle(PrimaryButton())
                .disabled(!valid)
            }
        }
        .padding(28)
        .frame(width: 520)
        .background(Theme.surface)
    }
}

struct Toast: View {
    let text: String

    var body: some View {
        HStack(spacing: 10) {
            Image(systemName: "checkmark.circle.fill").foregroundStyle(Theme.accent)
            Text(text).font(.system(size: 13)).foregroundStyle(Theme.text)
        }
        .padding(.horizontal, 16).padding(.vertical, 12)
        .background(Theme.surface, in: RoundedRectangle(cornerRadius: 14))
        .overlay(RoundedRectangle(cornerRadius: 14).stroke(Theme.line))
    }
}
